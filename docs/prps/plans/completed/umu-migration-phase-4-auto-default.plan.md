# Plan: umu-launcher Migration — Phase 4 (Auto default + exported-script parity)

## Summary

Flip `UmuPreference::Auto` so it prefers `umu-run` when available (today Auto falls through to direct Proton), align the preview diagnostic + stale doc comments + Settings UI copy with the new semantics, and teach `build_exec_line` in `export/launcher.rs` to emit a runtime `command -v umu-run` probe with `"$PROTON" run` fallback so exported trainer scripts get the same "use umu when present" behavior. Steam-applaunch and Flatpak-Steam-trainer paths remain force-opted-out via the existing `force_no_umu_for_launch_request` predicate; no new dispatch method, no new settings fields.

## User Story

As a hybrid CrossHook user with `umu-run` installed on my host (native or Flatpak), I want CrossHook to prefer `umu-run` by default for non-Steam Windows launches, so that my games pick up protonfixes and the standard umu runtime without me having to change `UmuPreference` from `Auto` to `Umu` manually — and so exported launcher scripts I share continue to work on hosts without umu.

## Problem → Solution

Today (Phase 3): `UmuPreference::Auto` is behaviorally identical to `Proton` — `should_use_umu` at `launch/script_runner.rs:1024` gates strictly on `preference == Umu`, and `build_exec_line` at `export/launcher.rs:521-551` hardcodes `exec "$PROTON" run …`. Auto adopters never actually get umu, and exported scripts never get umu even when the runtime does.

Desired (Phase 4): `Auto` resolves to `umu-run` when `resolve_umu_run_path().is_some()` and falls back to direct Proton otherwise. Exported scripts probe `command -v umu-run` at runtime and exec whichever is available, preserving share-ability across hosts. `UmuPreference::Proton` remains the explicit opt-out for titles where `umu-run` is known to break (for example, _The Witcher 3_).

## Metadata

- **Complexity**: Medium (6 files touched in crosshook-core + frontend)
- **Source PRD**: `docs/prps/prds/umu-launcher-migration.prd.md`
- **PRD Phase**: Phase 4 — Auto-default + exported-script parity
- **Estimated Files**: 6 production files (3 Rust + 1 Rust/TypeScript boundary comment + 2 TS/TSX) + 3 test sites (same 3 Rust files)
- **Research Dispatch**: Agent team (`--team`, 3 researchers under `prpp-umu-phase-4-auto`)

## Persistence / Usability

Classifies Phase 4 data and reviewer expectations for migration, offline behavior, and visibility. No new SQLite tables or columns; no new TOML keys.

| Item                                                                                                 | Classification                                                                                        | Migration / backward compatibility                                                                                                                                                                                                                             | Offline / degraded fallback                                                                                                                                                                          | User visibility & editability                                                                                                                                                                   |
| ---------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **`umu_preference` (`UmuPreference`)**                                                               | **User-editable preference** (`settings.toml` via `AppSettingsData`)                                  | Existing persisted values unchanged (`Auto`, `Umu`, `Proton`). Phase 4 only changes **runtime resolution** of `Auto` (prefer `umu-run` when `resolve_umu_run_path()` succeeds). Omitted key still deserializes as `Auto` (existing test in `settings/mod.rs`). | No network required. If `umu-run` is absent from PATH, `Auto` falls back to direct Proton (same as explicit `Proton` for launch mechanics). Explicit `Umu` keeps existing warn-on-fallback behavior. | **Editable**: Settings → Runner (global default). **Visible**: label updated to “Auto (umu when available, else Proton)”; launch preview shows `umu_decision.reason` for the active resolution. |
| **Auto-default launch path** (`should_use_umu` for `Auto` when umu present)                          | **Ephemeral runtime** (derived at launch from preference + host PATH + `resolve_umu_run_path()`)      | N/A — not stored.                                                                                                                                                                                                                                              | Without `umu-run` on PATH, `Auto` uses direct Proton; Steam-applaunch / Flatpak trainer routing still force-opt-out via `force_no_umu_for_launch_request` (unchanged).                               | **Indirect**: user sees outcome in launch preview and logs, not a separate stored field.                                                                                                        |
| **Exported-script parity** (`build_exec_line` runtime `command -v umu-run` probe + `$PROTON` branch) | **Ephemeral at export** (script file content on disk when user exports; not CrossHook DB metadata)    | Exported scripts from older builds remain valid; new exports embed the probe. Re-export to refresh script shape.                                                                                                                                               | On a host with no `umu-run`, script falls through to `"$PROTON" run` at **run** time (PATH on the machine running the script). CrossHook offline does not block export.                              | **Visible** when opening the generated script; **not** a separate in-app setting beyond existing export UX.                                                                                     |
| **Preview copy / TS doc-comments** (`umu_decision.reason`, settings types)                           | **Ephemeral UI** (computed strings + source docs; optional local settings file already covered above) | No migration.                                                                                                                                                                                                                                                  | Preview reflects the same resolution rules as launch when backend is available.                                                                                                                      | Launch Preview panel and Settings copy.                                                                                                                                                         |

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order. File-concurrency rule is respected: no two tasks in the same batch touch the same file.

| Batch | Tasks                        | Depends On | Parallel Width |
| ----- | ---------------------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3, 1.4, 1.5, 1.6 | —          | 6              |
| B2    | 2.1, 2.2, 2.3                | B1         | 3              |
| B3    | 3.1                          | B2         | 1              |

- **Total tasks**: 10
- **Total batches**: 3
- **Max parallel width**: 6

---

## UX Design

### Before (Phase 3)

```
Settings → Runner (global default)
  ┌─────────────────────────────────────────┐
  │ Auto (Phase 3 → Proton)      [selected] │ ← user picks "Auto", still gets Proton
  │ Umu (umu-launcher)                      │
  │ Proton (direct)                         │
  └─────────────────────────────────────────┘

Launch preview (proton_run method):
  umu_decision:
    requested_preference = Auto
    will_use_umu         = false
    reason               = "preference = Auto — Phase 3 resolves Auto to direct Proton"
    command              = "/opt/proton/GE-Proton9-20/proton" run /tmp/game.exe

Exported trainer script: always
  exec "$PROTON" run "$trainer_host_path"
```

### After (Phase 4)

```
Settings → Runner (global default)
  ┌───────────────────────────────────────────────────────────┐
  │ Auto (umu when available, else Proton)       [selected]   │
  │ Umu (umu-launcher)                                        │
  │ Proton (direct — compatibility fallback)                  │
  └───────────────────────────────────────────────────────────┘

Launch preview (proton_run method, umu-run on PATH):
  umu_decision:
    requested_preference = Auto
    will_use_umu         = true
    reason               = "using umu-run at /usr/bin/umu-run"
    command              = umu-run /tmp/game.exe

Launch preview (proton_run method, umu-run NOT on PATH):
  umu_decision:
    requested_preference = Auto
    will_use_umu         = false
    reason               = "preference = Auto but umu-run was not found on the backend PATH — falling back to direct Proton"
    command              = "/opt/proton/GE-Proton9-20/proton" run /tmp/game.exe

Exported trainer script:
  if command -v umu-run >/dev/null 2>&1; then
    exec "${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}" umu-run "$trainer_host_path"
  else
    exec "${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}" "$PROTON" run "$trainer_host_path"
  fi
```

### Interaction Changes

| Touchpoint                                               | Before                                                       | After                                                                                             | Notes                                                                         |
| -------------------------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Settings → Runner label                                  | "Auto (Phase 3 → Proton)"                                    | "Auto (umu when available, else Proton)"                                                          | String-only; no wire/serde change.                                            |
| Launch Preview `umu_decision.reason` (Auto, umu present) | "preference = Auto — Phase 3 resolves Auto to direct Proton" | "using umu-run at `<path>`"                                                                       | Uses the existing `(_, _, true)` arm — new Auto path naturally folds into it. |
| Launch Preview `umu_decision.reason` (Auto, umu absent)  | "preference = Auto — Phase 3 resolves Auto to direct Proton" | "preference = Auto but umu-run was not found on the backend PATH — falling back to direct Proton" | New arm.                                                                      |
| Exported trainer `.sh` (umu present on target host)      | `exec "$PROTON" run …`                                       | `exec umu-run …`                                                                                  | Runtime probe; Phase 4 is the first exported-script `command -v` use.         |
| Exported trainer `.sh` (umu absent on target host)       | `exec "$PROTON" run …`                                       | `exec "$PROTON" run …` (unchanged)                                                                | Shared-script safety — hosts without umu keep working.                        |
| Flatpak-Steam trainer + Steam applaunch                  | direct Proton                                                | direct Proton (unchanged)                                                                         | `force_no_umu_for_launch_request` is untouched.                               |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                          | Lines                                     | Why                                                                                                                                                 |
| -------------- | ----------------------------------------------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `docs/prps/prds/umu-launcher-migration.prd.md`                                | §Phase 4, §Decisions, §Technical Approach | PRD scope, acceptance signals, explicit non-goals (Steam profiles, bundled umu, Proton removal retired).                                            |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 1004-1069                                 | `force_no_umu_for_launch_request`, `should_use_umu`, `warn_on_umu_fallback`, `proton_path_dirname` — Phase 4 flip point.                            |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 408-527                                   | `build_proton_game_command` — how `use_umu` threads into `PROTON_VERB=waitforexitandrun`, `PROTONPATH`, `GAMEID`, program path.                     |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 529-604                                   | `build_proton_trainer_command_with_umu_override` — trainer variant uses `PROTON_VERB=runinprefix`; `force_no_umu` suppresses fallback warn.         |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`            | 162-181                                   | `UmuDecisionPreview` struct — IPC-serialized shape surfaced in Launch Preview UI.                                                                   |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`            | 443-476                                   | `build_umu_decision_preview` — reason-string match that hardcodes "Phase 3 resolves Auto to direct Proton".                                         |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`           | 521-551                                   | `build_exec_line` — 4-branch (gamescope × network_isolation) fn that Phase 4 rewrites to emit the `command -v umu-run` probe.                       |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`           | 490-519                                   | `build_gamescope_script_block` + `_GS_PREFIX` pattern — template for the new `_UMU_PREFIX`/`_UMU_RUN` runtime selection.                            |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | 2614-2856                                 | All existing umu tests — fixture idioms (`ScopedCommandSearchPath`, empty-tempdir), naming, `command_env_value` usage.                              |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`            | 1700-1800                                 | Inline preview umu tests — the pattern for asserting on `effective_command`, `umu_run_path`, and `umu_decision.reason`.                             |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`           | 1245-1310                                 | `network_isolation_enabled_generates_runtime_probe_and_exec` — multi-assert runtime-probe test shape Phase 4 mirrors.                               |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`            | 63-66                                     | `LaunchRequest.umu_preference` stale doc-comment ("resolves to direct Proton in Phase 3").                                                          |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`              | 134-163, 218-246                          | `UmuPreference` enum + `AppSettingsData.umu_preference` default; back-compat test at line 609.                                                      |
| P1 (important) | `docs/prps/plans/completed/umu-migration-phase-1-proton-verb-hygiene.plan.md` | all                                       | Phase 1 conventions — `PROTON_VERB` hygiene, `WINE_ENV_VARS_TO_CLEAR` discipline.                                                                   |
| P1 (important) | `docs/prps/plans/umu-migration-phase-3-umu-opt-in.plan.md`                    | all                                       | Phase 3 conventions this phase extends — `should_use_umu` contract, builder split, fixture idioms, test naming.                                     |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`    | 613-653                                   | `resolve_umu_run_path` — how umu is located on Flatpak (`/run/host/env/PATH`) vs. native (`$PATH`); still the authoritative source for the backend. |
| P2 (reference) | `src/crosshook-native/src/components/SettingsPanel.tsx`                       | 1065-1080                                 | Where the stale "Auto (Phase 3 → Proton)" label lives.                                                                                              |
| P2 (reference) | `src/crosshook-native/src/types/settings.ts`                                  | 37-38                                     | TS doc-comment mirroring the stale Phase 3 Auto semantics.                                                                                          |
| P2 (reference) | `.git-cliff.toml`                                                             | 39-61                                     | Conventional-commit → `### Features` routing — a `feat(launch): …` commit is how Phase 4 surfaces in `CHANGELOG.md`.                                |
| P2 (reference) | `CHANGELOG.md`                                                                | 27-37                                     | Phase 3 precedent for how a phase ships in release notes (no manual edit; git-cliff handles it).                                                    |

## External Documentation

| Topic                         | Source                                                 | Key Takeaway                                                                                                                             |
| ----------------------------- | ------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| umu-launcher default behavior | <https://github.com/Open-Wine-Components/umu-launcher> | v1.4.0 stable; Lutris 0.5.20 + Heroic 2.16 default to umu — Phase 4 brings CrossHook in line with the ecosystem.                         |
| `command -v` POSIX semantics  | POSIX.1-2017 §Shell Built-In Utilities                 | `command -v <name> >/dev/null 2>&1` returns 0 if name is an executable in PATH, 1 otherwise. Portable across `/bin/sh`, bash, zsh, dash. |

_(No library/API additions in Phase 4 — only internal pattern changes.)_

---

## Patterns to Mirror

Code patterns discovered in the codebase during research. Follow these exactly.

### NAMING_CONVENTION — snake_case verb-first helpers, `pub(crate)` for cross-module, private for single-file

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:1004,1012,1062
pub(crate) fn force_no_umu_for_launch_request(request: &LaunchRequest) -> bool { … }
pub(crate) fn should_use_umu(request: &LaunchRequest, force_no_umu: bool) -> (bool, Option<String>) { … }
fn warn_on_umu_fallback(request: &LaunchRequest) { … }
```

### NAMING_CONVENTION — boolean call-site flags always tagged with `/*param=*/` doc comments

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:401-405
build_proton_trainer_command_with_umu_override(
    &direct_request,
    log_path,
    /*force_no_umu=*/ true,
)
```

### NAMING_CONVENTION — exported-script shell prefix arrays use `SCREAMING_SNAKE` with leading underscore

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:505-516
r#"if [[ -z "${GAMESCOPE_WAYLAND_DISPLAY:-}" ]]; then
  _GS_PREFIX=(gamescope "${_GAMESCOPE_ARGS[@]}" --)
else
  _GS_PREFIX=()
fi"#,
```

### ERROR_HANDLING — degraded-fallback log is `info!` for decision, `warn!` for user-requested-but-missing

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:1062-1069
fn warn_on_umu_fallback(request: &LaunchRequest) {
    if request.umu_preference == UmuPreference::Umu {
        tracing::warn!("umu preference requested but umu-run is not on PATH; falling back to direct Proton for this launch");
    }
}
```

### ERROR_HANDLING — impossible preview combinations labeled `(bug)`, never `panic!` / `unreachable!`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/preview.rs:462-465
(UmuPreference::Umu, Some(_), false) => {
    "preference = Umu and umu-run found, but should_use_umu returned false (bug)".to_string()
}
```

### LOGGING_PATTERN — structured `tracing::info!` on every `should_use_umu` branch

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:1033-1037
tracing::info!(
    preference = ?request.umu_preference,
    umu_run_path = %path,
    "should_use_umu: using umu-run"
);
```

### RUNTIME_PROBE_PATTERN — exported-script "probe → set prefix → exec fallback" shape

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:531-540
r#"_NET_PREFIX=()
if unshare --net true >/dev/null 2>&1; then
  _NET_PREFIX=(unshare --net)
else
  echo "[CrossHook] WARNING: unshare --net unavailable — launching without network isolation" >&2
fi
"#
```

### TEST_STRUCTURE — umu-present fixture via `ScopedCommandSearchPath`

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:2615-2620
let dir = tempfile::tempdir().unwrap();
let umu_stub = dir.path().join("umu-run");
std::fs::write(&umu_stub, "#!/bin/sh\nexit 0\n").unwrap();
std::fs::set_permissions(&umu_stub, std::fs::Permissions::from_mode(0o755)).unwrap();
let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());
```

### TEST_STRUCTURE — umu-absent fixture: empty tempdir + guard

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:2660-2663
fn proton_game_command_falls_back_to_proton_when_umu_preferred_but_missing_on_path() {
    let dir = tempfile::tempdir().unwrap();
    let _guard = crate::launch::test_support::ScopedCommandSearchPath::new(dir.path());
```

### TEST_STRUCTURE — `command_env_value` for env assertions (positive AND negative)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:1117-1126
fn command_env_value(command: &Command, key: &str) -> Option<String> {
    command.as_std().get_envs().find_map(|(env_key, env_value)| {
        (env_key == std::ffi::OsStr::new(key))
            .then(|| env_value.map(|v| v.to_string_lossy().into_owned()))
    }).flatten()
}
```

### TEST_STRUCTURE — exported-script multi-assert runtime-probe shape

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:1258-1271
assert!(content.contains("if unshare --net true"),     "script should probe unshare availability: {content}");
assert!(content.contains("_NET_PREFIX=(unshare --net)"), "script should set _NET_PREFIX on success: {content}");
assert!(content.contains("WARNING: unshare --net unavailable"), "script should warn on failure: {content}");
assert!(content.contains(r#"exec "${_NET_PREFIX[@]}" "$PROTON" run"#), "exec line should use _NET_PREFIX array: {content}");
```

### TEST_STRUCTURE — raw-string bash assertions on exported script content

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:888,1010,1169
assert!(script_content.contains(r#"exec "$PROTON" run "$staged_trainer_windows_path""#));
assert!(script_content.contains(r#"exec "$PROTON" run "$trainer_host_path""#));
assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" "$PROTON" run "$trainer_host_path""#));
```

---

## Files to Change

| File                                                                     | Action | Justification                                                                                                                                                                             |
| ------------------------------------------------------------------------ | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` | UPDATE | Flip `should_use_umu` so `Auto` + `resolve_umu_run_path().is_some()` uses umu. Rename/update `auto_preference_resolves_to_proton_in_phase_3` and add Auto-with-umu tests.                 |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`       | UPDATE | Rewrite the `(UmuPreference::Auto, _, false)` reason-string arm so it explains the umu-absent fallback accurately (no more "Phase 3 resolves Auto"). Extend inline preview tests.         |
| `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`       | UPDATE | Doc-comment on `LaunchRequest.umu_preference` says "resolves to direct Proton in Phase 3" — update to reflect Auto-prefers-umu semantics.                                                 |
| `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`      | UPDATE | Rework `build_exec_line` to emit a `command -v umu-run` probe with `_UMU_PREFIX`-style array selection; update 6 existing `exec "$PROTON" run` assertions and add dual-branch assertions. |
| `src/crosshook-native/src/components/SettingsPanel.tsx`                  | UPDATE | Replace "Auto (Phase 3 → Proton)" label with "Auto (umu when available, else Proton)".                                                                                                    |
| `src/crosshook-native/src/types/settings.ts`                             | UPDATE | Update TS doc-comment on `SettingsSaveRequest.umu_preference` to match new Auto semantics.                                                                                                |

_No new files; no file deletions._

## NOT Building

- **Not** removing `UmuPreference::Proton` — Phase 6 is retired per PRD; direct Proton remains a supported compatibility escape hatch (e.g. _The Witcher 3_).
- **Not** removing the direct-Proton builder branches in `build_proton_game_command` / `build_proton_trainer_command`. Both `use_umu` and `!use_umu` code paths stay.
- **Not** adding a fourth dispatch method (`METHOD_UMU_RUN`) — we branch inside existing builders (Phase 3 decision, preserved).
- **Not** migrating Steam profiles or Steam-applaunch helper trainers — `force_no_umu_for_launch_request` stays untouched and still routes those to direct Proton.
- **Not** changing the onboarding readiness dialog (`onboarding/readiness.rs:125-151`) from Info to actionable — that's Phase 5.
- **Not** adding `--filesystem=xdg-data/umu:create` to the Flatpak manifest — Phase 5.
- **Not** wiring HTTP `umu-database` lookups — Phase 3b (#247) owns that; Phase 4 does not change the existing CSV coverage surfacing.
- **Not** introducing new TOML settings fields — Auto semantics flip is pure-behavior; no schema change; `AppSettingsData.umu_preference` default stays `Auto`.
- **Not** editing `CHANGELOG.md` by hand — git-cliff renders the `feat(launch): …` commit under `### Features` via `.git-cliff.toml:51`; the commit title IS the release note.
- **Not** changing `warn_on_umu_fallback` — it still only warns when the user explicitly asked for `Umu` and it was missing. Auto users silently fall back (that's the expected Auto behavior).

---

## Step-by-Step Tasks

Each task lists its BATCH, ACTION, IMPLEMENT sketch, MIRROR reference, GOTCHA, and VALIDATE command. Within a batch, tasks are fully independent (no shared files).

### Task 1.1: Flip `should_use_umu` so `Auto` prefers umu when present — Depends on [none]

- **BATCH**: B1
- **ACTION**: Edit `should_use_umu` in `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (~lines 1012-1049) so `UmuPreference::Auto` behaves like `Umu` for the resolution step (`resolve_umu_run_path`), while `UmuPreference::Proton` stays an unconditional direct-Proton.
- **IMPLEMENT**: Replace the `if request.umu_preference != UmuPreference::Umu { … return (false, None); }` gate with a match on `request.umu_preference`. `Proton` → `info!("should_use_umu: preference = Proton → direct Proton")`, return `(false, None)`. `Umu | Auto` → fall through to `resolve_umu_run_path()`. Preserve the existing `force_no_umu` early-return at the top, and keep the existing `tracing::info!` calls for the umu-found / umu-missing branches. Update the doc-comment at lines 992-1001 to read "`request.umu_preference == UmuPreference::Umu || UmuPreference::Auto`" for the positive clause.
- **MIRROR**: `LOGGING_PATTERN — structured tracing::info!`; `ERROR_HANDLING — fail-early degraded fallback (builder side)`.
- **IMPORTS**: none new (`UmuPreference` already in scope via `use crate::settings::UmuPreference`).
- **GOTCHA**: Do NOT remove `warn_on_umu_fallback` or its callers. Auto falling back to Proton when umu is absent is SILENT by design — only explicit `Umu` warns. Keep `force_no_umu` precedence above everything else (Flatpak + Steam trainer must still short-circuit before preference is even inspected).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core should_use_umu` — existing `auto_preference_resolves_to_proton_in_phase_3` test will fail after this change (intentionally, addressed by Task 2.1).

### Task 1.2: Rewrite Auto reason-string arm in `build_umu_decision_preview` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Edit `build_umu_decision_preview` in `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` (lines 443-476) so the `(UmuPreference::Auto, _, false)` arm accurately describes the fallback (not "Phase 3 resolves…"), and the umu-found branch (which already folds Auto into `(_, _, true)`) remains unchanged.
- **IMPLEMENT**: Change `(UmuPreference::Auto, _, false) => "preference = Auto — Phase 3 resolves Auto to direct Proton".to_string()` to `(UmuPreference::Auto, _, false) => "preference = Auto but umu-run was not found on the backend PATH — falling back to direct Proton".to_string()`. Leave `(UmuPreference::Proton, _, false)`, `(UmuPreference::Umu, None, false)`, and the labeled-`(bug)` arm exactly as they are. The `(_, _, true)` arm (line 449-452) already handles Auto + umu-found correctly because it matches on `will_use_umu`.
- **MIRROR**: `ERROR_HANDLING — impossible preview combinations labeled (bug)` — keep the match exhaustive; do not add a catch-all.
- **IMPORTS**: none.
- **GOTCHA**: The match is on `(requested, umu_run_path.as_deref(), will_use_umu)`. After Task 1.1 lands, `will_use_umu` returns `true` when `Auto` + umu-found. Verify the tuple still routes to the `(_, _, true)` arm by re-reading the match order — `(_, _, true)` MUST be first (it is, line 449).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core build_umu_decision` (expect pre-existing Phase-3 auto preview tests to fail here — fixed by Task 2.2).

### Task 1.3: Refresh `LaunchRequest.umu_preference` doc-comment — Depends on [none]

- **BATCH**: B1
- **ACTION**: Edit `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` lines 63-66 to replace the stale "Defaults to `UmuPreference::Auto`, which resolves to direct Proton in Phase 3." doc-comment with accurate Phase 4 semantics.
- **IMPLEMENT**: Change to `/// Defaults to UmuPreference::Auto, which prefers umu-run when available and falls back to direct Proton otherwise.` on one rustdoc line; keep the `#[serde(default)]` attribute and field visibility unchanged.
- **MIRROR**: n/a (doc-comment-only change).
- **IMPORTS**: none.
- **GOTCHA**: Do not change the field type, default, or serde attribute. The `settings_backward_compat_without_umu_preference` test at `settings/mod.rs:609` pins the TOML default to `Auto` — keep that invariant.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` succeeds.

### Task 1.4: Introduce umu runtime probe in `build_exec_line` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Rewrite `build_exec_line` in `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs` (lines 521-551) so every emitted `exec` line becomes a runtime selection between `umu-run "$target"` and `"$PROTON" run "$target"`, probed once via `command -v umu-run >/dev/null 2>&1`.
- **IMPLEMENT**: Emit a preamble that sets `_UMU_RUN` as a single-element command array when umu is present, else an empty array, and split the final `exec` into two forms gated by an `if`:

  ```bash
  _UMU_AVAILABLE=0
  if command -v umu-run >/dev/null 2>&1; then
    _UMU_AVAILABLE=1
  fi
  if [ "$_UMU_AVAILABLE" = "1" ]; then
    exec "${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}" umu-run "$target"
  else
    exec "${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}" "$PROTON" run "$target"
  fi
  ```

  Make the emitted prefixes conditional on the existing `gamescope_enabled` / `network_isolation` flags — keep the 4-branch behavior that decides which prefix arrays appear. Wrap the two `exec` lines inside the same `if`/`else` regardless of branch. The `command -v umu-run` probe is emitted **once** (not per prefix combination). Note: the function has no host-side `umu_preference` input — exported scripts are always share-able, so the probe is purely runtime.

- **MIRROR**: `RUNTIME_PROBE_PATTERN — exported-script "probe → set prefix → exec fallback" shape`; `NAMING_CONVENTION — exported-script shell prefix arrays use SCREAMING_SNAKE with leading underscore` (use `_UMU_AVAILABLE`, NOT `umu_available`).
- **IMPORTS**: none (pure `String` assembly).
- **GOTCHA**:
  (a) When `gamescope_enabled=false` AND `network_isolation=false`, the `_GS_PREFIX`/`_NET_PREFIX` arrays are NOT declared earlier in the script — emit the `exec` lines WITHOUT those `"${_GS_PREFIX[@]}"` / `"${_NET_PREFIX[@]}"` tokens in that branch. Match the current 4-branch structure exactly.
  (b) Keep `target_path` already-quoted as its caller passes it (e.g. `"$trainer_host_path"`, `"$staged_trainer_windows_path"`) — do not re-quote.
  (c) Do NOT emit a `tracing::warn!` equivalent; exported scripts silently prefer umu when present. No `echo "[CrossHook] WARNING: ..."` line, to match PRD's "silent Auto fallback" contract.
  (d) Place the `command -v umu-run` probe BEFORE any `exec` so the script exits via `exec` on all branches.
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` succeeds; 6 existing `exec "$PROTON" run` test assertions in the same file will need updating in Task 2.3.

### Task 1.5: Update Settings UI `Auto` label — Depends on [none]

- **BATCH**: B1
- **ACTION**: Edit `src/crosshook-native/src/components/SettingsPanel.tsx` line 1075 to replace the stale "Auto (Phase 3 → Proton)" label.
- **IMPLEMENT**: Change `{ value: 'auto', label: 'Auto (Phase 3 → Proton)' }` to `{ value: 'auto', label: 'Auto (umu when available, else Proton)' }`. Leave the `umu` and `proton` option objects unchanged. Do not rename the `value` field — serde wire stays `"auto"`.
- **MIRROR**: n/a (string-only change).
- **IMPORTS**: none.
- **GOTCHA**: Do not rename the `value: 'auto'` key — `UmuPreference`'s `#[serde(rename_all = "snake_case")]` matches on that exact string. Renaming it breaks persistence compatibility for every existing user.
- **VALIDATE**: `./scripts/lint.sh` passes (Biome) and browser-dev renders the Settings panel without TS errors: `./scripts/dev-native.sh --browser` → open Settings → Runner dropdown shows the new label.

### Task 1.6: Update TS type doc-comment for `UmuPreference` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Edit `src/crosshook-native/src/types/settings.ts` lines 37-38 to align the JSDoc with Phase 4 Auto semantics.
- **IMPLEMENT**: Replace the JSDoc so it describes Phase 4 semantics: `auto` (umu when available, else Proton), `umu` (always umu-run), and `proton` (always direct Proton)—instead of the old Phase 3 wording (“Phase 3 → Proton”, opt-in/disable). Do not change the field name, type union, or `DEFAULT_APP_SETTINGS.umu_preference = 'auto'` at line 99.
- **MIRROR**: n/a (comment-only change).
- **IMPORTS**: none.
- **GOTCHA**: If Phase 3 types appear duplicated in `src/crosshook-native/src/types/profile.ts` and `src/crosshook-native/src/types/launch.ts`, verify — those also carry the `UmuPreference` union but may not carry the same doc-comment. If they do reference Phase 3 semantics, update them identically. Otherwise leave them alone.
- **VALIDATE**: `./scripts/lint.sh` passes (Biome TS check).

### Task 2.1: Update Rust tests in `script_runner.rs` for Auto semantics — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: In `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (tests module, ~line 2694+), replace the now-stale `auto_preference_resolves_to_proton_in_phase_3` test with two tests covering the new Auto behavior, and extend the trainer-test matrix correspondingly.
- **IMPLEMENT**:
  1. Remove `auto_preference_resolves_to_proton_in_phase_3` (lines 2694-2727).
  2. Add `auto_preference_uses_umu_when_umu_run_present`: fixture = tempdir + `umu-run` stub + `ScopedCommandSearchPath`; `UmuPreference::Auto`; assert `program.ends_with("/umu-run")`, `command_env_value(&command, "PROTON_VERB") == Some("waitforexitandrun")`, and `PROTONPATH == Some(dirname(proton_path))`.
  3. Add `auto_preference_falls_back_to_proton_when_umu_run_missing`: fixture = empty tempdir + `ScopedCommandSearchPath`; `UmuPreference::Auto`; assert `!program.ends_with("umu-run")`, `command_env_value(&command, "PROTONPATH") == None`, `command_env_value(&command, "PROTON_VERB") == None` (consistent with existing Proton-fallback tests).
  4. Add an equivalent trainer pair: `auto_preference_uses_umu_trainer_when_present` (assert `PROTON_VERB=runinprefix`) and `auto_preference_trainer_falls_back_to_proton_when_missing`.
  5. Assert no `warn!` is emitted for the Auto-fallback path. (If the module already captures tracing output, add a negative assertion; otherwise rely on `warn_on_umu_fallback`'s guard clause being covered by the `Umu`-preference tests at lines 2661-2690.)
- **MIRROR**: `TEST_STRUCTURE — umu-present fixture`; `TEST_STRUCTURE — umu-absent fixture`; `TEST_STRUCTURE — command_env_value for env assertions`.
- **IMPORTS**: `tempfile`, `std::fs`, `std::os::unix::fs::PermissionsExt` (already imported in the tests module — re-use).
- **GOTCHA**:
  (a) Do NOT also touch the 4 existing `UmuPreference::Umu` tests (lines 2614-2690) — they continue to cover explicit Umu opt-in.
  (b) Do NOT touch `proton_preference_always_uses_direct_proton` (line 2729) — it verifies `UmuPreference::Proton` is still an explicit escape hatch.
  (c) Do NOT touch `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred` — Steam opt-out is unchanged.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --lib launch::script_runner::tests` — all tests green including the 4 new ones.

### Task 2.2: Update preview tests for new Auto reason strings — Depends on [1.2]

- **BATCH**: B2
- **ACTION**: In `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` tests module (~lines 1700-1800), update or add tests that assert the Auto reason strings.
- **IMPLEMENT**:
  1. Locate any existing test asserting the string "Phase 3 resolves Auto to direct Proton" and delete it.
  2. Add `auto_preference_preview_reports_using_umu_when_umu_run_present`: fixture = tempdir + `umu-run` stub + `ScopedCommandSearchPath`; request with `UmuPreference::Auto` and `METHOD_PROTON_RUN`; build preview; assert `preview.umu_decision.as_ref().unwrap().will_use_umu == true` and `preview.umu_decision.as_ref().unwrap().reason.starts_with("using umu-run at ")`.
  3. Add `auto_preference_preview_explains_fallback_when_umu_missing`: fixture = empty tempdir + `ScopedCommandSearchPath`; `UmuPreference::Auto`; assert `will_use_umu == false` and `reason == "preference = Auto but umu-run was not found on the backend PATH — falling back to direct Proton"`.
- **MIRROR**: `TEST_STRUCTURE — umu-present fixture`; `TEST_STRUCTURE — umu-absent fixture`.
- **IMPORTS**: already available in the module.
- **GOTCHA**: `UmuDecisionPreview` is only populated when `resolved_method == ResolvedLaunchMethod::ProtonRun` (preview.rs:410-413). The request fixture MUST set `method = METHOD_PROTON_RUN` — otherwise `umu_decision` is `None` and the test unwraps will panic with an unhelpful message.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --lib launch::preview::tests` — green, including the 2 new Auto tests.

### Task 2.3: Update exported-script tests for dual-branch probe — Depends on [1.4]

- **BATCH**: B2
- **ACTION**: In `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs` tests module, update the 6 `exec "$PROTON" run` assertions at lines 888, 1010, 1148, 1169, 1185, 1306 to assert BOTH branches (umu success + Proton fallback) of the new probe, and add a standalone probe-shape test mirroring `network_isolation_enabled_generates_runtime_probe_and_exec`.
- **IMPLEMENT**:
  1. For each of the 6 call sites, keep the existing `exec "$PROTON" run …` fallback assertion AND add a sibling umu-branch assertion using the same prefix chain. Example at line 888:

     ```rust
     assert!(script_content.contains(r#"exec "$PROTON" run "$staged_trainer_windows_path""#));
     assert!(script_content.contains(r#"exec umu-run "$staged_trainer_windows_path""#));
     ```

     At line 1169:

     ```rust
     assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" "$PROTON" run "$trainer_host_path""#));
     assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" umu-run "$trainer_host_path""#));
     ```

     At line 1306 (gamescope + network isolation):

     ```rust
     assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}" "$PROTON" run"#));
     assert!(content.contains(r#"exec "${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}" umu-run"#));
     ```

  2. Add `build_exec_line_emits_umu_probe_and_dual_exec`: fixture = minimal trainer request; call `build_trainer_script_content(&request, "Test Game")`; multi-assert:

     ```rust
     assert!(content.contains("command -v umu-run"), "script should probe for umu-run");
     assert!(content.contains("_UMU_AVAILABLE=1"), "script should mark umu availability");
     assert!(content.contains(r#"exec "$PROTON" run"#), "fallback exec must remain");
     assert!(content.contains("exec "), "at least one exec line present");
     ```

  3. Update the line 1270 test (`network_isolation_enabled_generates_runtime_probe_and_exec`) to also assert the umu sibling: `assert!(content.contains(r#"exec "${_NET_PREFIX[@]}" umu-run"#));`.

- **MIRROR**: `TEST_STRUCTURE — exported-script multi-assert runtime-probe shape`; `TEST_STRUCTURE — raw-string bash assertions`.
- **IMPORTS**: none new.
- **GOTCHA**:
  (a) The exported-script probe is UNCONDITIONAL — emitted for all 4 `(gamescope × network_isolation)` branches. Every one of the 6 existing assertions now has a sibling.
  (b) Keep raw-string literals (`r#"…"#`) — the assertions include double quotes and dollar signs.
  (c) `build_trainer_script_content` returns `String` (not `Result`), so no `.unwrap()` needed (confirmed at `export/launcher.rs:888`).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core --lib export::launcher::tests` — all assertions green.

### Task 3.1: Full-repo verification — Depends on [2.1, 2.2, 2.3]

- **BATCH**: B3
- **ACTION**: Run the full lint + test battery to confirm no regressions in unrelated crates or the frontend.
- **IMPLEMENT**:
  1. `./scripts/lint.sh` — Rust (`cargo fmt --check`, `cargo clippy -D warnings`), TypeScript (Biome), shell (shellcheck).
  2. `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` — full crosshook-core suite.
  3. `./scripts/dev-native.sh --browser` — smoke the Settings panel new label and confirm no console errors (browser-only mode; no Rust toolchain required).
- **MIRROR**: n/a.
- **IMPORTS**: n/a.
- **GOTCHA**:
  (a) CI runs `./scripts/lint.sh` in `-D warnings` mode; any new clippy hint from the `should_use_umu` match rewrite must be addressed before merging (e.g. redundant match arms).
  (b) No frontend test framework exists per CLAUDE.md — browser smoke is the only UI verification.
- **VALIDATE**: All three commands exit 0; `gh pr view` later should label the PR with `area:launch`, `type:feature`.

---

## Testing Strategy

### Unit Tests

| Test                                                                              | Input                                               | Expected Output                                                                                              | Edge Case?      |
| --------------------------------------------------------------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | --------------- |
| `auto_preference_uses_umu_when_umu_run_present`                                   | `Auto`, umu stub on PATH, proton_run method         | program ends with `/umu-run`; `PROTON_VERB=waitforexitandrun`; `PROTONPATH=<proton dirname>`; `GAMEID` set   | Happy path      |
| `auto_preference_falls_back_to_proton_when_umu_run_missing`                       | `Auto`, empty PATH, proton_run method               | program does NOT end with `umu-run`; no `PROTONPATH` / `PROTON_VERB` / `GAMEID`; no `tracing::warn!`         | Yes             |
| `auto_preference_uses_umu_trainer_when_present`                                   | `Auto`, umu stub on PATH, proton_run trainer        | trainer program ends with `/umu-run`; `PROTON_VERB=runinprefix`                                              | Happy path      |
| `auto_preference_trainer_falls_back_to_proton_when_missing`                       | `Auto`, empty PATH, proton_run trainer              | trainer direct Proton; no umu env                                                                            | Yes             |
| `umu_preference_still_forces_umu_when_present` (pre-existing)                     | `Umu`, umu stub on PATH                             | program ends with `/umu-run`                                                                                 | Regression      |
| `proton_preference_always_uses_direct_proton` (pre-existing)                      | `Proton`, umu stub on PATH                          | program does NOT end with `umu-run`                                                                          | Escape-hatch    |
| `flatpak_steam_trainer_command_never_uses_umu_even_when_preferred` (pre-existing) | Flatpak + Steam applaunch + `Umu`                   | direct Proton (opt-out via `force_no_umu_for_launch_request`)                                                | Regression      |
| `auto_preference_preview_reports_using_umu_when_umu_run_present`                  | `Auto` + umu stub + ProtonRun method                | `umu_decision.will_use_umu == true`; reason starts with `"using umu-run at "`                                | Happy path      |
| `auto_preference_preview_explains_fallback_when_umu_missing`                      | `Auto` + empty PATH + ProtonRun method              | `will_use_umu == false`; reason string exactly matches new copy                                              | Yes             |
| `build_exec_line_emits_umu_probe_and_dual_exec`                                   | minimal trainer request                             | output contains `command -v umu-run`, `_UMU_AVAILABLE=1`, both `exec "$PROTON" run` and `exec umu-run` lines | Happy path      |
| 6 existing exec-line assertions (lines 888, 1010, 1148, 1169, 1185, 1306)         | Respective fixture + `build_trainer_script_content` | BOTH `exec "$PROTON" run …` fallback AND sibling `exec … umu-run …` line present                             | Regression      |
| `network_isolation_enabled_generates_runtime_probe_and_exec` (extended)           | network_isolation=true                              | adds `exec "${_NET_PREFIX[@]}" umu-run` sibling assertion                                                    | Combined probes |
| `settings_backward_compat_without_umu_preference` (pre-existing)                  | TOML without `umu_preference`                       | Deserialized `AppSettingsData.umu_preference == UmuPreference::Auto`                                         | Regression      |

### Edge Cases Checklist

- [x] `Auto` + umu present + `proton_run` method → uses umu (Task 1.1 + 2.1)
- [x] `Auto` + umu absent + `proton_run` method → direct Proton, no warn! (Task 1.1 + 2.1)
- [x] `Auto` + umu present + `steam_applaunch` method → still direct Proton (already covered by `force_no_umu_for_launch_request` + existing Steam opt-out tests; add a preview-side regression test if missing)
- [x] `Auto` + Flatpak + trainer-only + Steam applaunch → still direct Proton (pre-existing `force_no_umu_for_launch_request` test)
- [x] `Proton` (explicit) + umu present → still direct Proton (`proton_preference_always_uses_direct_proton`)
- [x] `Umu` (explicit) + umu absent → direct Proton + `tracing::warn!` fires (unchanged; `warn_on_umu_fallback`)
- [x] Exported script run on host WITH umu → execs `umu-run`
- [x] Exported script run on host WITHOUT umu → execs `"$PROTON" run`
- [x] Exported script + gamescope only → probe+dual-exec still wraps both branches in `"${_GS_PREFIX[@]}"`
- [x] Exported script + network_isolation only → probe+dual-exec still wraps both branches in `"${_NET_PREFIX[@]}"`
- [x] Exported script + gamescope + network_isolation → probe+dual-exec wraps in `"${_GS_PREFIX[@]}" "${_NET_PREFIX[@]}"` (gamescope outermost per existing contract)
- [x] TOML without `umu_preference` key → loads as `Auto` (pre-existing back-compat test at `settings/mod.rs:609`)

### Manual Validation

- [ ] Launch CrossHook browser-dev (`./scripts/dev-native.sh --browser`). Confirm Settings → Runner dropdown shows "Auto (umu when available, else Proton)".
- [ ] In Launch Preview on a non-Steam `proton_run` profile with `Auto`: verify `umu_decision.reason` matches the new copy.
- [ ] Build a full native binary (`./scripts/build-native.sh --binary-only`). On a native host with `umu-run` installed, export a trainer script (via existing export flow) and run it on a machine WITHOUT `umu-run` on PATH — confirm it falls through to `"$PROTON" run`. This is the cross-host shareability check called out in the PRD's "Should" capability.

---

## Validation Commands

### Static Analysis

```bash
./scripts/lint.sh
```

EXPECT: Zero Rust `cargo fmt` drift, zero `cargo clippy -D warnings`, zero Biome TS/TSX errors, zero shellcheck errors.

### Unit Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: All existing tests pass; 7 new tests pass (4 in `launch::script_runner::tests`, 2 in `launch::preview::tests`, 1 net-new in `export::launcher::tests`; sibling assertions amended in `export::launcher::tests` do not add new `#[test]` entries).

### Type Check (TypeScript)

```bash
./scripts/lint.sh
```

EXPECT: Biome passes; no new TS errors introduced by `SettingsPanel.tsx` / `types/settings.ts` edits.

### Browser Dev Smoke

```bash
./scripts/dev-native.sh --browser
```

EXPECT: Settings panel loads with the new "Auto (umu when available, else Proton)" label; no console errors.

### Full Binary Build (optional, CI covers it)

```bash
./scripts/build-native.sh --binary-only
```

EXPECT: AppImage builds; exported trainer script from a non-Steam profile contains both `command -v umu-run` probe and dual-branch `exec` lines.

---

## Acceptance Criteria

- [ ] `UmuPreference::Auto` resolves to `umu-run` iff `resolve_umu_run_path().is_some()` at launch time (both game and trainer builders).
- [ ] `UmuPreference::Proton` continues to force direct Proton regardless of umu presence.
- [ ] `UmuPreference::Umu` + missing `umu-run` still falls back with a single `tracing::warn!` per builder call.
- [ ] Flatpak + Steam-applaunch + trainer-only STILL routes to direct Proton (`force_no_umu_for_launch_request` path untouched).
- [ ] Launch Preview `umu_decision.reason` accurately describes Auto's resolution for all three combinations (umu-present uses shared `(_, _, true)` arm, umu-absent uses new arm, Proton/Umu arms unchanged).
- [ ] Exported trainer scripts emit a runtime `command -v umu-run` probe and execute `umu-run` when available, `"$PROTON" run` otherwise — regardless of `gamescope_enabled` / `network_isolation`.
- [ ] Settings UI and TS doc-comments reflect Phase 4 semantics; no user's persisted `umu_preference` value needs migration.
- [ ] All validation commands (lint, cargo test, browser dev smoke) pass.
- [ ] PR title is a Conventional Commit `feat(launch): …` — routes under `### Features` in `CHANGELOG.md` via git-cliff.
- [ ] PR is labeled `type:feature`, `area:launch` and linked to GitHub issue #257 (Phase 4 tracker) + #239 (Phase 4 implementation).

## Completion Checklist

- [ ] Code follows patterns discovered in Phase 3 — `should_use_umu` contract, builder split, `/*param=*/` call-site annotations, `ScopedCommandSearchPath` fixture idiom.
- [ ] Error handling: Auto fallback is silent (no `warn!`); explicit `Umu`+missing keeps its single `warn!`; no `panic!`/`unreachable!` introduced.
- [ ] Logging: every `should_use_umu` branch still emits a `tracing::info!` with structured fields (`preference`, `umu_run_path`).
- [ ] Tests follow the module's naming convention (`<subject>_<verb>_when_<condition>`); reuse `command_env_value`; new tests added in the same test module as the code they cover.
- [ ] No hardcoded paths in tests — continue using `tempfile::tempdir()` + `ScopedCommandSearchPath`.
- [ ] Frontend label change is localized to the `options` array; wire-level `value: 'auto'` is UNCHANGED.
- [ ] Stale doc-comments purged — `launch/request.rs:63-66`, `types/settings.ts:37-38`, preview reason string.
- [ ] Self-contained — no further codebase exploration required during implementation.

## Risks

| Risk                                                                                                  | Likelihood | Impact                                                | Mitigation                                                                                                                                                                                                                                                                                    |
| ----------------------------------------------------------------------------------------------------- | ---------- | ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Existing Auto users unexpectedly see new behavior (positive umu launches for previously-Proton runs)  | M          | User perception: "what changed?" without release note | Ship a clear `feat(launch): enable umu-launcher by default for non-Steam launches (Phase 4)` commit message that lands as a release-note line; PRD calls this the "umu is here" moment.                                                                                                       |
| A title breaks under umu for a user whose Auto previously got them to a working Proton path           | M          | User-reported regression against `area:launch`        | `UmuPreference::Proton` remains the documented escape hatch; Settings label now explicitly offers "Proton (direct — compatibility fallback)". Document in release note. _The Witcher 3_ is the known case.                                                                                    |
| Exported-script `command -v umu-run` behaves differently in non-bash `/bin/sh` (dash)                 | L          | Shared script fails on Debian/Ubuntu default shell    | `command -v` is POSIX standard; `>/dev/null 2>&1` and `[ "$VAR" = "1" ]` are POSIX-safe. Scripts already use `[[ … ]]` (bash-specific) elsewhere in the same file, so bash shebang is assumed. Confirm the shebang at `build_exec_line`'s caller is `#!/usr/bin/env bash` (or `#!/bin/bash`). |
| `cargo clippy -D warnings` flags the rewritten `match request.umu_preference` for redundant arms      | L          | CI fails                                              | Keep the match minimal: `Proton =>` returns, `Umu \| Auto =>` falls through (rust 1.53+ syntax). Run `cargo clippy` locally in Task 3.1.                                                                                                                                                      |
| Test duplication explodes in `export::launcher::tests` (each of 6 sites gets a sibling umu assertion) | L          | Review noise                                          | Accept the mechanical doubling — matches how Phase 3 split command-shape tests in `script_runner.rs`. No helper extraction needed.                                                                                                                                                            |
| Frontend label change missed in a sibling file (`profile.ts`, `launch.ts`) leaves docs inconsistent   | L          | Stale docs                                            | Task 1.6 GOTCHA directs implementor to sibling type files; code review can catch residual drift.                                                                                                                                                                                              |

## Notes

- **Why not extract a helper for the dual-branch exec line?** The four-way `(gamescope × network_isolation)` matrix in `build_exec_line` already resists over-abstraction. Introducing a helper would only save ~6 lines across the 4 branches while hiding the shell shape. Mirror Phase 3's bias: keep `build_exec_line` inlined and readable, add the probe + dual-exec inline.
- **Why no `Option<UmuPreference>` migration?** `AppSettingsData.umu_preference` is already a typed-enum field with `#[serde(default)] = Auto`. Existing users on `Auto` silently gain umu support on next launch without touching their TOML, per PRD's "zero user action required" clause.
- **Why no CHANGELOG edit?** `.git-cliff.toml:51` routes `^feat` commits under `### Features`, and `scripts/prepare-release.sh` regenerates `CHANGELOG.md` from git-cliff at tag time. The Conventional Commit message IS the release-note source of truth.
- **Why no warning on Auto fallback?** Auto is the default and is expected to quietly pick whichever runner is available. Warning every launch on hosts without umu would be noise. Explicit `Umu` preference warns because the user asked for a specific behavior that could not be honored — that's the distinction.
- **Post-ship observation window**: PRD defines success as "2 minor releases after Phase 4 ship" with qualitative `area:launch` issue-count decline + positive sentiment. No telemetry wired in this phase (per PRD deferral to #246).
- **Phase 3b prerequisite**: PRD lists "#263 + #247 landed AND 2-week observation clean" as a prerequisite for Phase 4 default-on. Confirm before merging; if observation window not yet elapsed, hold the PR or ship as unflipped-by-default and flip in a follow-up.
- **Linked GitHub issues**: tracker #257, implementation #239.
