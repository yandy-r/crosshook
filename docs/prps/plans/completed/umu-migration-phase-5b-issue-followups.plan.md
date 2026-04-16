# Plan: umu-launcher Migration — Phase 5b (Issue follow-ups #242 / #244 / #245)

## Summary

Phase 5b is the follow-up phase that lands actionable outcomes for the three Phase-5-gated Open Questions the PRD tracked as GitHub issues. Phase 5 shipped the Flatpak manifest change and the first-run install-guidance plumbing; this plan extends that plumbing with (a) the resolved Flathub decision for `org.openwinecomponents.umu.umu-launcher` (confirmed not published — #242), (b) a gamescope → pressure-vessel teardown fallback that replaces silent watchdog stand-down with exe-name-based host-PID discovery (#244), and (c) a new Steam-Deck gaming-mode caveats surface in onboarding (#245), all built on the existing readiness payload + dismissal pattern.

## User Story

As a hybrid CrossHook user on a Flatpak host or on Steam Deck gaming mode, I want (1) clear install guidance that reflects the real Flathub availability of umu-launcher, (2) reliable cleanup of Wine/game processes when gamescope closes even if the watchdog can't see the capture file, and (3) upfront warnings about the documented SteamOS 3.7+ caveats (Shader Pre-Caching, Steam overlay z-order, HDR), so umu adoption doesn't surprise me with broken teardown or undocumented gotchas.

## Problem → Solution

- **#242 current**: the PRD and onboarding dialog both assume Flathub availability is unknown; copy may suggest installing from Flathub that does not exist. → **Solution**: mark the Open Question resolved as "not published", keep distro-aware install commands, add Faugus Launcher Flathub app (`io.github.Faugus.faugus-launcher`) as a trusted install vehicle link in the guidance payload's description/alternatives.
- **#244 current**: when the gamescope PID capture file is never resolved, `gamescope_watchdog` logs a warn and stands down — any residual Wine/game PIDs inside the pressure-vessel child tree are left to the OS. No host-namespace fallback runs by exe name. → **Solution**: add a stand-down fallback that reuses the existing host-ps BFS walker (`collect_host_descendant_pids` + `is_host_descendant_process_running`) to locate the game executable by `comm`/`cmdline`, then run the same SIGTERM → wait → SIGKILL sequence on the discovered PID. Also add structured outcome tracing so field reports are diagnosable.
- **#245 current**: no Steam-Deck / SteamOS detection exists; onboarding has no surface to warn about documented gaming-mode caveats. → **Solution**: add a sibling `is_steam_deck()` helper next to `is_flatpak()`, extend `ReadinessCheckResult` with an optional `SteamDeckCaveats` payload mirroring the `UmuInstallGuidance` shape, render a dedicated review-summary section with a persistent dismiss button backed by a new `steam_deck_caveats_dismissed_at` RFC3339 setting.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/umu-launcher-migration.prd.md`
- **PRD Phase**: Phase 5b (issue follow-ups — #242, #244, #245). Phase 5 is already complete.
- **Estimated Files**: 16

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks                        | Depends On | Parallel Width |
| ----- | ---------------------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3, 1.4, 1.5, 1.6 | —          | 6              |
| B2    | 2.1, 2.2, 2.3, 2.4, 2.5      | B1         | 5              |
| B3    | 3.1                          | B2         | 1              |
| B4    | 4.1, 4.2, 4.3                | B3         | 3              |
| B5    | 5.1                          | B4         | 1              |

- **Total tasks**: 16
- **Total batches**: 5
- **Max parallel width**: 6

Batch construction rules honoured:

- No two tasks in the same batch touch the same file.
- All cross-cutting Rust types (Settings field, onboarding result struct, platform helper) land in B1.
- `readiness.rs`, `watchdog.rs`, `commands/settings.rs`, `commands/onboarding.rs`, and `mocks/handlers/onboarding.ts` each appear in exactly one task.
- UI wiring touches the hook first (B3), then three independent UI sites in B4.

---

## UX Design

### Before (today, post-Phase-5)

```
┌─────────────────────────── Onboarding → Review ──────────────────────────┐
│ ✓ Steam install found                                                    │
│ ✓ Proton build(s) detected                                               │
│ ℹ  umu-run not found                                                      │
│                                                                          │
│ ┌─── UMU launcher install guidance ──────────────────────────────────┐   │
│ │ Install umu-launcher for the easiest non-Steam launches.            │   │
│ │ Command: yay -S umu-launcher-git                                    │   │
│ │ Docs: https://github.com/Open-Wine-Components/umu-launcher          │   │
│ │ [Copy command] [Open docs] [Dismiss]                                │   │
│ └─────────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│  — no Steam Deck caveats surface —                                       │
│  — no Faugus trusted install-vehicle hint —                              │
└──────────────────────────────────────────────────────────────────────────┘
```

### After (Phase 5b)

```
┌─────────────────────────── Onboarding → Review ──────────────────────────┐
│ ✓ Steam install found                                                    │
│ ✓ Proton build(s) detected                                               │
│ ℹ  umu-run not found                                                      │
│                                                                          │
│ ┌─── UMU launcher install guidance ──────────────────────────────────┐   │
│ │ Install umu-launcher for the easiest non-Steam launches.            │   │
│ │ Command: yay -S umu-launcher-git                                    │   │
│ │ Docs: https://github.com/Open-Wine-Components/umu-launcher          │   │
│ │ Also trusted: Faugus Launcher (Flathub) bundles umu-launcher.       │   │
│ │   flatpak install flathub io.github.Faugus.faugus-launcher          │   │
│ │ [Copy command] [Open docs] [Dismiss]                                │   │
│ └─────────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│ ┌─── Steam Deck gaming-mode caveats ─────────────────────────── (new) ┐  │
│ │ CrossHook works on Steam Deck desktop mode today. In gaming mode    │  │
│ │ you may hit these upstream issues on SteamOS 3.7+:                  │  │
│ │  • Black screen until Shader Pre-Caching completes                  │  │
│ │  • Steam overlay can render below the game                          │  │
│ │  • HDR + gamescope + Flatpak regression on SteamOS 3.7.13           │  │
│ │ [Open docs] [Dismiss]                                               │  │
│ └─────────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────┘
```

No visual UX change for #244 — it is a background-watchdog behavior change. The only user-observable delta is: when gamescope closes under Flatpak and the capture file never landed, leftover Wine/game processes are now killed instead of being left hanging (logged, not user-facing).

### Interaction Changes

| Touchpoint                    | Before                                                       | After                                                                                                   | Notes                                                                                  |
| ----------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Review → UMU guidance section | Distro command + docs + dismiss                              | Adds a short "Also trusted: Faugus Launcher" line inside `description`; docs URL unchanged              | Keep same three buttons; no new button for Faugus (copy/open via description text).    |
| Review → Steam Deck caveats   | Not rendered                                                 | New `<section>` below UMU guidance, only when `is_steam_deck()` and not dismissed                       | Same BEM pattern as UMU guidance. One "Open docs" button + one "Dismiss" ghost button. |
| Gamescope teardown (Flatpak)  | Watchdog stands down silently when capture file not resolved | Watchdog falls back to exe-name host-PID lookup, then SIGTERM→SIGKILL; standing-down becomes last-ditch | Structured tracing records which branch ran (`capture_file`, `exe_fallback`, `none`).  |

---

## Mandatory Reading

Files that MUST be read before implementing. All paths are relative to the repo root.

| Priority       | File                                                                            | Lines                     | Why                                                                                                                                          |
| -------------- | ------------------------------------------------------------------------------- | ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`        | 1-367                     | Phase-5 guidance payload + distro detection + nag dismissal — #245 mirrors this exact structure and #242 extends `build_umu_install_advice`. |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs`              | 1-50                      | `UmuInstallGuidance`, `HealthIssue`, and `ReadinessCheckResult` type definitions — new `SteamDeckCaveats` lives here.                        |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/platform.rs`                    | 1-260                     | `is_flatpak()` pattern + `host_command_with_env` wrapper — template for `is_steam_deck()`.                                                   |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs`             | 1-660                     | Entire watchdog flow: host-ps BFS, `resolve_watchdog_target` stand-down, SIGTERM/SIGKILL + descendants, pure unit tests.                     |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`                | 134-260, 420-440, 620-670 | Enum serde pattern, `install_nag_dismissed_at` precedent, `SettingsStore::update`, back-compat tests.                                        |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/onboarding.rs`                     | 1-140                     | `check_readiness` + `dismiss_umu_install_nag` + tests using function-pointer signature assertions.                                           |
| P1 (important) | `src/crosshook-native/src-tauri/src/commands/settings.rs`                       | 1-430                     | Triple-state `Option<Option<T>>` merge + serialization contract tests.                                                                       |
| P1 (important) | `src/crosshook-native/src-tauri/src/lib.rs`                                     | 180-220, 380-410          | Startup onboarding-check emission + `invoke_handler` registration.                                                                           |
| P1 (important) | `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`            | 1-200                     | Section render pattern for guidance (copy/open-docs/dismiss triad + BEM classes).                                                            |
| P1 (important) | `src/crosshook-native/src/hooks/useOnboarding.ts`                               | 100-200                   | `readinessResult` derivation + optimistic dismiss patch.                                                                                     |
| P1 (important) | `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts`                     | 1-140                     | Browser-mode IPC mock pattern — `verify:no-mocks` fails without mirrored handler for every new command.                                      |
| P1 (important) | `src/crosshook-native/src/types/onboarding.ts`                                  | 1-40                      | TS type shape mirroring (snake_case preserved across IPC boundary).                                                                          |
| P1 (important) | `src/crosshook-native/src/types/settings.ts`                                    | 1-60                      | TS settings mirror where `install_nag_dismissed_at` lives today.                                                                             |
| P2 (reference) | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`      | 709-770, 19-21            | `resolve_umu_run_path` + `probe_flatpak_host_umu_candidates` + `FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT`.                                       |
| P2 (reference) | `src/crosshook-native/src-tauri/src/commands/launch.rs`                         | 290-400, 1060-1130        | End-to-end launch flow: capture-path gating → watchdog spawn → diagnostic-method log parsing.                                                |
| P2 (reference) | `docs/prps/prds/umu-launcher-migration.prd.md`                                  | all                       | Source PRD — Decisions Log + Open Questions #1, #3, #4 need updating after this plan ships.                                                  |
| P2 (reference) | `docs/prps/reports/umu-migration-phase-5-flatpak-host-shared-runtime-report.md` | all                       | Phase-5 implementation delta — gives concrete precedent for dismissal + guidance wiring.                                                     |
| P2 (reference) | `packaging/flatpak/dev.crosshook.CrossHook.yml`                                 | 1-80                      | Confirms `--filesystem=xdg-data/umu:create` already landed in Phase 5; no manifest change needed in Phase 5b.                                |

## External Documentation

| Topic                                                     | Source                                                                                                                                           | Key Takeaway                                                                                                                                           |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `org.openwinecomponents.umu.umu-launcher` Flathub listing | `https://flathub.org/apps/org.openwinecomponents.umu.umu-launcher` (404 at plan time) and upstream issue `Open-Wine-Components/umu-launcher#335` | NOT PUBLISHED at Flathub as of plan date. Resolves PRD Open Question #1 negatively — no one-click Flathub install UX.                                  |
| Faugus Launcher (trusted install vehicle)                 | `https://flathub.org/apps/io.github.Faugus.faugus-launcher`                                                                                      | Published, bundles umu-launcher 1.4+. App ID is `io.github.Faugus.faugus-launcher`. Link-only; do not probe Faugus's private `~/.var/app` for umu-run. |
| SteamOS 3.7 Shader Pre-Caching black-screen regression    | SteamOS 3.7+ release notes / ValveSoftware/SteamOS-on-deck issue tracker (linked through upstream Open-Wine-Components discussion)               | Gaming mode may black-screen a gamescope+umu launch until Steam's Shader Pre-Caching finishes; document, don't attempt workaround.                     |
| Steam overlay z-order regression                          | ValveSoftware gamescope issue tracker                                                                                                            | Steam overlay occasionally renders below the game under gamescope+Flatpak; documented caveat, not a CrossHook bug.                                     |
| SteamOS 3.7.13 HDR + gamescope + Flatpak regression       | ValveSoftware gamescope-session issue tracker                                                                                                    | HDR toggles broken for sandboxed gamescope on SteamOS 3.7.13; mention in caveat body as a versioned note.                                              |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION — RFC3339 dismissal setting

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:220-222
/// RFC 3339 timestamp of when the user dismissed the umu install nag;
/// `None` = not dismissed.
pub install_nag_dismissed_at: Option<String>,
```

Apply to the new Steam-Deck field: `pub steam_deck_caveats_dismissed_at: Option<String>,`.

### NAMING_CONVENTION — platform detection helper

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/platform.rs:24-31
pub fn is_flatpak() -> bool {
    is_flatpak_with(FLATPAK_ID_ENV, Path::new(FLATPAK_INFO_PATH))
}
fn is_flatpak_with(env_key: &str, info_path: &Path) -> bool {
    std::env::var_os(env_key).is_some() || info_path.exists()
}
```

Sibling API: `pub fn is_steam_deck() -> bool` + `fn is_steam_deck_from_sources(env, os_release: Option<&str>) -> bool` for test injection.

### NAMING_CONVENTION — TS snake_case preserved

```typescript
// SOURCE: src/crosshook-native/src/types/onboarding.ts:3-22
export interface UmuInstallGuidance {
  install_command: string;
  docs_url: string;
  description: string;
}
```

New TS type `SteamDeckCaveats` keeps Rust snake_case field names (`docs_url`, `items`, `description`).

### ERROR_HANDLING — Tauri command result contract

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/onboarding.rs:28-36
#[tauri::command]
pub fn dismiss_umu_install_nag(store: State<'_, SettingsStore>) -> Result<(), String> {
    store.update(|s| {
        s.install_nag_dismissed_at = Some(chrono::Utc::now().to_rfc3339());
        Ok::<(), String>(())
    }).map_err(|e| e.to_string())?
}
```

`dismiss_steam_deck_caveats` follows this exact shape — atomic `update` + RFC3339 timestamp + stringified error.

### ERROR*HANDLING — readiness Option guard & apply*\*

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs:358-367
pub fn apply_install_nag_dismissal(
    result: &mut ReadinessCheckResult,
    install_nag_dismissed_at: &Option<String>,
) {
    if install_nag_dismissed_at.is_some() {
        result.umu_install_guidance = None;
    }
}
```

Add sibling `apply_steam_deck_caveats_dismissal` with the same shape.

### LOGGING_PATTERN — structured tracing fields

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:476-487
tracing::debug!(
    target_pid = request.gamescope_pid,
    use_umu,
    umu_run_path = umu_run_path.as_deref().unwrap_or(""),
    "building proton game launch"
);
```

New watchdog tracing must use the same `field = value` structured style, no `target=` scoping.

### LOGGING_PATTERN — watchdog warn on stand-down

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs:128-134
tracing::warn!(
    observed_gamescope_pid,
    capture_path = %path.display(),
    "gamescope watchdog: host pid capture file was never resolved; standing down"
);
None
```

Replace with a new emit that reports the fallback outcome: `fallback = "exe_name" | "capture_file" | "none"` field + `discovered_pid` + `game_exe`.

### REPOSITORY_PATTERN — atomic settings mutation

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/settings/mod.rs:418-431
let _guard = self.io_lock.lock().expect("settings mutex poisoned");
fs::create_dir_all(&self.base_path)?;
let mut settings = self.load_unlocked()?;
let result = mutator(&mut settings);
if result.is_ok() { self.save_unlocked(&settings)?; }
```

Every persisted change goes through `SettingsStore::update`, never raw `load → edit → save`.

### SERVICE_PATTERN — guidance gating match arm

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs:297-336
match umu_path {
    Some(ref p) => { /* ok */ },
    None if is_flatpak => {
        let advice = build_umu_install_advice(detect_host_distro_family());
        checks.push(HealthIssue { field: "umu_run_available".into(), /* Info */ .. });
        umu_install_guidance = Some(advice.guidance);
    },
    None => { /* non-flatpak info */ },
}
```

Steam-Deck caveat gating extends `evaluate_checks_inner` with `is_steam_deck: bool` and populates `steam_deck_caveats: Option<SteamDeckCaveats>` unconditionally on Deck when not dismissed.

### SERVICE_PATTERN — host-ps BFS walker (reuse for #244)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs:85-105
for line in output.lines() {
    if let Some((pid, ppid)) = parse_host_pid_and_ppid(line) {
        children_map.entry(ppid).or_default().push(pid);
    }
}
collect_descendant_pids_from_children_map(root_pid, &children_map)
```

Fallback walks from gamescope PID → Wine game PID by `comm`/`cmdline` match via the existing `is_host_descendant_process_running` helper; no new parser.

### SERVICE_PATTERN — browser-mode mock handler

```typescript
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/onboarding.ts:60-89
map.set('dismiss_umu_install_nag', async (): Promise<null> => {
  store.settings.install_nag_dismissed_at = new Date().toISOString();
  return null;
});
```

Add a parallel `dismiss_steam_deck_caveats` handler. `check_readiness` mock must now also return `steam_deck_caveats` when a new toggle (or `getActiveToggles().showSteamDeckCaveats`) is set.

### TEST_STRUCTURE — readiness Flatpak/native matrix

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs:523-609
let result = evaluate_checks_inner(&[steam_root], &[proton], None, false);
assert!(matches!(umu_check.severity, HealthIssueSeverity::Info));
assert!(umu_check.remediation.is_empty());
assert!(result.umu_install_guidance.is_none());
assert!(result.all_passed);
```

Steam-Deck matrix: `{umu present/absent} × {is_flatpak true/false} × {is_steam_deck true/false} × {dismissed true/false}` = 16 rows, asserting caveats presence and `all_passed` invariance (caveats are Info-only).

### TEST_STRUCTURE — os-release closure injection

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs:611-639
let content = read_host_os_release_with(
    true,
    |_| None,
    || Some("ID=cachyos\nID_LIKE=arch\n".to_string())
);
```

`is_steam_deck_from_sources(env, os_release)` tests use the same closure-injection style — zero filesystem touches at test time.

### TEST_STRUCTURE — Tauri command signature freeze

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/onboarding.rs:98-139
let _ = check_readiness
    as fn(State<'_, SettingsStore>) -> Result<ReadinessCheckResult, String>;
```

Every new `#[tauri::command]` gets a one-line type-coercion to lock its signature in tests.

### TEST_STRUCTURE — settings triple-state merge

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/settings.rs:314-358
assert_eq!(merged.install_nag_dismissed_at, Some("2026-04-15T12:00:00Z".into()),
    "absent field must preserve");
request.install_nag_dismissed_at = Some(None);
assert!(merged.install_nag_dismissed_at.is_none(), "explicit null must clear");
```

New `steam_deck_caveats_dismissed_at` gets absent-preserve, set, and explicit-null-clear tests plus JSON serialization containment.

### TEST_STRUCTURE — watchdog pure unit coverage

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs:595-652
let children_map = HashMap::from([(10, vec![11,12]),(11, vec![13]),(12, vec![14])]);
assert_eq!(collect_descendant_pids_from_children_map(10, &children_map),
    vec![11,12,13,14]);
```

`resolve_watchdog_target` fallback tests stay map/string-level with fake `comm` lookups; no `/proc` or process spawning in tests.

---

## Files to Change

| File                                                                       | Action | Justification                                                                                                                                                                                           |
| -------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/platform.rs`               | UPDATE | Add `is_steam_deck()` and `is_steam_deck_from_sources()` next to `is_flatpak()` (#245).                                                                                                                 |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | UPDATE | Add `steam_deck_caveats_dismissed_at: Option<String>` field + back-compat test (#245).                                                                                                                  |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs`         | UPDATE | Add `SteamDeckCaveats` struct + `steam_deck_caveats: Option<SteamDeckCaveats>` field on `ReadinessCheckResult` (#245).                                                                                  |
| `src/crosshook-native/src/types/onboarding.ts`                             | UPDATE | Mirror `SteamDeckCaveats` + extend `ReadinessCheckResult` union (#245).                                                                                                                                 |
| `src/crosshook-native/src/types/settings.ts`                               | UPDATE | Mirror `steam_deck_caveats_dismissed_at` field (#245).                                                                                                                                                  |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | UPDATE | Extend `probe_flatpak_host_umu_candidates` with the Faugus Launcher host-umu path probe (#242).                                                                                                         |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`   | UPDATE | Extend `evaluate_checks_inner` with `is_steam_deck: bool`, build `SteamDeckCaveats`, add Faugus trust-vehicle line to `build_umu_install_advice`, add `apply_steam_deck_caveats_dismissal` (#242/#245). |
| `src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs`        | UPDATE | Add stand-down fallback: walk host-ps from observed gamescope PID to game exe by `comm`/`cmdline`, promote to shutdown target. Add structured tracing for fallback outcome (#244).                      |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`                  | UPDATE | Extend IPC request/merge with triple-state `steam_deck_caveats_dismissed_at` (#245).                                                                                                                    |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs`                | UPDATE | Add `#[tauri::command] dismiss_steam_deck_caveats`; apply caveats dismissal in `check_readiness` (#245).                                                                                                |
| `src/crosshook-native/src-tauri/src/lib.rs`                                | UPDATE | Register `commands::onboarding::dismiss_steam_deck_caveats` in `invoke_handler` (#245).                                                                                                                 |
| `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts`                | UPDATE | Mirror `dismiss_steam_deck_caveats`; extend `check_readiness` mock with `steam_deck_caveats` payload (#245).                                                                                            |
| `src/crosshook-native/src/hooks/useOnboarding.ts`                          | UPDATE | Expose `steamDeckCaveats` + `dismissSteamDeckCaveats` with the same optimistic patch pattern (#245).                                                                                                    |
| `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`       | UPDATE | Render a sibling `<section>` for Steam-Deck caveats using the BEM class `crosshook-onboarding-wizard__steam-deck-caveats` (#245).                                                                       |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`                 | UPDATE | Prop-drill `steamDeckCaveats` + `onDismissSteamDeckCaveats` into the review step (#245).                                                                                                                |
| `docs/prps/prds/umu-launcher-migration.prd.md`                             | UPDATE | Mark Open Question #1 resolved (Flathub not published), note #3 and #4 resolved under Phase 5b; update Decisions Log + issue table (#242/#244/#245).                                                    |

## NOT Building

- **One-click Flathub install action** for `org.openwinecomponents.umu.umu-launcher`. Flathub listing is confirmed 404; `#242` resolves as "stay on distro-aware commands + link to Faugus Launcher trust vehicle". No new `open flathub:` action.
- **Faugus-sandbox umu-run probing.** Faugus's umu is inside its own Flatpak sandbox at `~/.var/app/io.github.Faugus.faugus-launcher/...`. CrossHook's sandbox cannot read that path without finicky overrides; we surface Faugus as an install vehicle (link + text), not a path probe.
- **New watchdog supervisor.** `#244` extends the existing `gamescope_watchdog`; it does NOT introduce a separate watchdog process, DB-backed state, or resumable supervisor. The in-process `Arc<AtomicBool>` pattern stays.
- **In-container Wine-PID capture script.** The fallback for `#244` uses host-ps exe-name discovery, not a second bash wrapper inside pressure-vessel. Adding a second capture script remains deferred as a follow-up only if field telemetry shows exe-name matching is insufficient.
- **Steam-Deck-specific automated workarounds.** `#245` is onboarding documentation, not fixes. CrossHook does not attempt to force Shader Pre-Caching on, raise the Steam overlay, or re-enable HDR — those are upstream regressions.
- **Recurring unsnooze on caveats dismissal.** One-shot RFC3339 timestamp matches the existing install-nag dismissal; per-release or N-day reappearance is out of scope. A future issue can add it if users ask.
- **Telemetry baseline.** PRD Open Question §5 (optional telemetry) remains deferred; Phase 5b only adds structured tracing logs, no upload/metrics.
- **Phase 5 manifest edits.** `--filesystem=xdg-data/umu:create` already landed in Phase 5; no change to `packaging/flatpak/dev.crosshook.CrossHook.yml`.

---

## Step-by-Step Tasks

### Task 1.1: Add `is_steam_deck()` helper to `platform.rs` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Add a `pub fn is_steam_deck() -> bool` next to `is_flatpak()` that returns true when any of: env `SteamDeck=1`, env `SteamOS=1`, `/etc/os-release` `ID=steamos` or `VARIANT_ID=steamdeck`, or `/run/host/etc/os-release` equivalent (Flatpak). Include a test-friendly `fn is_steam_deck_from_sources(env_lookup: impl Fn(&str) -> Option<String>, os_release: Option<&str>) -> bool` split.
- **IMPLEMENT**: Parse `ID`/`VARIANT_ID` using the same trimming/quote-stripping tokenisation used by `detect_host_distro_family_from_os_release`. Export the helper and the public fn in `platform::mod`; do NOT re-export from `launch::env` (it lives in platform for parity with `is_flatpak`).
- **MIRROR**: `NAMING_CONVENTION — platform detection helper`, `TEST_STRUCTURE — os-release closure injection`.
- **IMPORTS**: `use std::{env, path::Path};` — no new crate deps.
- **GOTCHA**: Do NOT rely on `/sys/class/dmi/id/product_name == "Jupiter"` — not readable from the Flatpak sandbox. os-release + env var is the authoritative signal. The env key `SteamDeck=1` is already in `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS` and must keep its pass-through behavior there; this new helper only _reads_ it.
- **VALIDATE**: `cargo test -p crosshook-core platform::tests -- --nocapture`. Add cases: `SteamDeck=1` env only, `VARIANT_ID=steamdeck` os-release only, `ID=steamos` os-release only, `ID=arch` → false, empty env + no os-release → false.

### Task 1.2: Add `steam_deck_caveats_dismissed_at` to `AppSettingsData` — Depends on [none]

- **BATCH**: B1
- **ACTION**: In `crates/crosshook-core/src/settings/mod.rs`, add a new optional RFC3339 timestamp field to `AppSettingsData` with `#[serde(default)]` semantics. Initialise in `Default` impl to `None`.
- **IMPLEMENT**: `pub steam_deck_caveats_dismissed_at: Option<String>` with a one-line doc comment matching the existing `install_nag_dismissed_at` style. Update the struct's `Default` to include the field.
- **MIRROR**: `NAMING_CONVENTION — RFC3339 dismissal setting`.
- **IMPORTS**: none new; existing serde derives cover it.
- **GOTCHA**: The struct has `#[serde(default)]` at struct level — absence is already handled. Do NOT add `#[serde(skip_serializing_if = ...)]` because `None` must serialize to preserve explicit-null-clear semantics the IPC merge relies on. Per CLAUDE.md, add the `backward_compat_without_steam_deck_caveats_dismissed_at` test in the same commit.
- **VALIDATE**: `cargo test -p crosshook-core settings:: -- --nocapture`. New tests: backward-compat-without-field, save-then-load roundtrip with `Some(...)` value.

### Task 1.3: Add `SteamDeckCaveats` and extend `ReadinessCheckResult` — Depends on [none]

- **BATCH**: B1
- **ACTION**: In `crates/crosshook-core/src/onboarding/mod.rs`, declare a new `pub struct SteamDeckCaveats { pub description: String, pub items: Vec<String>, pub docs_url: String }` with `#[derive(Debug, Clone, Serialize, Deserialize)]`. Add a new field `pub steam_deck_caveats: Option<SteamDeckCaveats>` to `ReadinessCheckResult`.
- **IMPLEMENT**: Mirror `UmuInstallGuidance` structure exactly. `items` is a `Vec<String>` of short caveat bullets (≤ 3 entries for MVP). Do NOT add severity enum; caveats are informational only.
- **MIRROR**: `NAMING_CONVENTION — TS snake_case preserved` (field names snake_case), `UmuInstallGuidance` shape from `onboarding/mod.rs:12-20`.
- **IMPORTS**: existing `serde::{Serialize, Deserialize}` already imported in the file.
- **GOTCHA**: Add `steam_deck_caveats: None` to every constructor/`..Default::default()` path. `ReadinessCheckResult` is constructed in at least `evaluate_checks_inner`, tests, and mock handlers — an incomplete init will break existing construction sites (explicit compile errors will point them out, do not add `#[serde(default)]` on the field to paper over this).
- **VALIDATE**: `cargo check -p crosshook-core` — expect compile errors at every `ReadinessCheckResult { ... }` literal; fix by adding `steam_deck_caveats: None`. Then `cargo test -p crosshook-core onboarding:: -- --nocapture`.

### Task 1.4: Mirror `SteamDeckCaveats` in `types/onboarding.ts` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend `src/types/onboarding.ts` with a TS interface for `SteamDeckCaveats` and a new `steam_deck_caveats: SteamDeckCaveats | null` field on `ReadinessCheckResult`.
- **IMPLEMENT**: `export interface SteamDeckCaveats { description: string; items: string[]; docs_url: string; }` — keep Rust snake_case verbatim. Append field to `ReadinessCheckResult` after `umu_install_guidance`.
- **MIRROR**: `NAMING_CONVENTION — TS snake_case preserved`.
- **IMPORTS**: none new; interface additions only.
- **GOTCHA**: `verify:no-mocks` sentinel in CI compares against the real IPC contract. Forgetting the `| null` union (defaulting to `SteamDeckCaveats` non-nullable) will break the mock handler's `null` return path.
- **VALIDATE**: `./scripts/lint.sh` — biome strict mode will catch shape mismatches downstream.

### Task 1.5: Mirror `steam_deck_caveats_dismissed_at` in `types/settings.ts` — Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend `src/types/settings.ts` with `steam_deck_caveats_dismissed_at: string | null` on the settings DTO + request shape.
- **IMPLEMENT**: One field insertion next to `install_nag_dismissed_at`, matching `string | null` union (serialized as RFC3339 or null).
- **MIRROR**: `NAMING_CONVENTION — TS snake_case preserved`.
- **IMPORTS**: none.
- **GOTCHA**: If the settings IPC uses a distinct `UpdateSettingsRequest` shape for the triple-state pattern, make sure to add the field there as well (the field uses the same `string | null` shape in TS even though Rust uses `Option<Option<String>>` — TS collapses the two layers into nullable-optional via the `?:` + `| null` combination).
- **VALIDATE**: `./scripts/lint.sh`.

### Task 1.6: Extend `probe_flatpak_host_umu_candidates` with Faugus Launcher paths — Depends on [none]

- **BATCH**: B1
- **ACTION**: In `crates/crosshook-core/src/launch/runtime_helpers.rs`, extend the Faugus-aware candidate list to include `~/.var/app/io.github.Faugus.faugus-launcher/data/umu/umu-run` (probe-only; skip with a tracing debug on miss). This is a trust-vehicle discovery, not a CrossHook-sandbox-visible path on stock Flatpak.
- **IMPLEMENT**: Add the Faugus candidate path after the existing home/user heuristics. Guard behind `if env::var_os("FLATPAK_ID").is_some()` like the rest. Emit `tracing::debug!(candidate = %path.display(), exists, "flatpak host umu candidate probe");` on each miss.
- **MIRROR**: `LOGGING_PATTERN — structured tracing fields`, `resolve_umu_run_path` at `runtime_helpers.rs:709-767`.
- **IMPORTS**: existing `std::env`, `std::path::Path`, `tracing` already imported.
- **GOTCHA**: This is NOT reachable from CrossHook's Flatpak sandbox by default. The discovery is speculative — if it ever succeeds in practice, it implies the user has an override in place. Do not add `--filesystem=~/.var/app/io.github.Faugus.faugus-launcher:ro` to the manifest; scope limits keep this a read-only side probe only.
- **VALIDATE**: `cargo test -p crosshook-core launch::runtime_helpers:: -- --nocapture`. Add a test that expects `None` when all candidates miss, and one that expects `Some(_)` when a tempdir'd candidate exists (using existing `ScopedCommandSearchPath` pattern).

### Task 2.1: Extend `readiness.rs` with Steam-Deck caveats + Faugus trust vehicle + tests — Depends on [1.1, 1.3, 1.6]

- **BATCH**: B2
- **ACTION**: Extend `evaluate_checks` + `evaluate_checks_inner` in `crates/crosshook-core/src/onboarding/readiness.rs` to accept and thread `is_steam_deck: bool`. Populate `result.steam_deck_caveats` when `is_steam_deck` is true (unconditionally of umu presence — caveats are runtime-environment warnings, not install blockers). Extend `build_umu_install_advice` output description to include a one-line Faugus trust-vehicle pointer: `"Also trusted: Faugus Launcher on Flathub (io.github.Faugus.faugus-launcher) bundles umu-launcher."`. Add `pub fn apply_steam_deck_caveats_dismissal(result: &mut ReadinessCheckResult, dismissed_at: &Option<String>)` mirroring `apply_install_nag_dismissal`.
- **IMPLEMENT**: Caveats content is a hard-coded English string for v1 — three `items` entries covering Shader Pre-Caching black-screen, Steam overlay z-order, and SteamOS 3.7.13 HDR regression. `docs_url` points at a CrossHook docs page or, v1, the upstream gamescope issue tracker as a `const STEAM_DECK_CAVEATS_DOCS_URL: &str = ...`. Caveats stay populated regardless of `is_flatpak` or umu presence.
- **MIRROR**: `SERVICE_PATTERN — guidance gating match arm`, `ERROR_HANDLING — readiness Option guard & apply_*`, `TEST_STRUCTURE — readiness Flatpak/native matrix`.
- **IMPORTS**: `use crate::platform::is_steam_deck;` from Task 1.1 at the call site in `evaluate_checks`; internal fn signature updates flow inward.
- **GOTCHA**: `all_passed` is computed from critical-severity counts — caveats are Info, must NOT flip `all_passed` to false. Add an explicit test proving that. Do NOT gate caveats on `!is_flatpak` — Steam Deck in desktop mode can run CrossHook native, gaming mode runs it as Flatpak, both show the caveats.
- **VALIDATE**: `cargo test -p crosshook-core onboarding:: -- --nocapture`. New tests: 16-row matrix for `{umu×flatpak×deck×dismissed}`; `build_umu_install_advice` description substring test for the Faugus line; `apply_steam_deck_caveats_dismissal` clears payload.

### Task 2.2: Add watchdog stand-down fallback + structured outcome tracing — Depends on [none]

- **BATCH**: B2
- **ACTION**: In `crates/crosshook-core/src/launch/watchdog.rs`, replace the silent `None` return at the stand-down branch (`watchdog.rs:128-134`) with a fallback that walks the host-ps tree starting from the spawned child PID (or from `observed_gamescope_pid` if known) and searches descendants for a process whose `comm`/`cmdline` matches the game exe name. If found, return `Some(ShutdownTarget { pid, host_namespace: true })`. If not found, keep the stand-down but replace the warn with a structured log carrying `fallback = "none"`, `observed_descendants = N`, `game_exe = <name>`.
- **IMPLEMENT**: New private `fn resolve_watchdog_target_by_exe_name(root_pid: i32, exe_name: &str) -> Option<ShutdownTarget>` that uses `collect_host_descendant_pids` + `is_host_descendant_process_running` primitives. Hoist `exe_name` into `resolve_watchdog_target` so the fallback has the candidate to match. Add three tracing emits representing the three exhaustive outcomes: `capture_file`, `exe_fallback`, `none` — each with a `fallback` field plus `discovered_pid`, `game_exe`, `observed_gamescope_pid`.
- **MIRROR**: `SERVICE_PATTERN — host-ps BFS walker (reuse for #244)`, `LOGGING_PATTERN — watchdog warn on stand-down`, `TEST_STRUCTURE — watchdog pure unit coverage`.
- **IMPORTS**: reuse existing `tracing`, `tokio::time`, internal `collect_host_descendant_pids`, `is_host_descendant_process_running`, `host_std_command`. No new crate deps.
- **GOTCHA**: `TASK_COMM_LEN=15` Linux kernel truncation means `comm` can be shortened (e.g. `FFXIV.exe` → `FFXIV.exe` is fine, but `FooBarBazGameLong.exe` → `FooBarBazGameLo`). `is_host_descendant_process_running` already falls back to `/proc/<pid>/cmdline` on comm miss — use the same helper, do NOT re-implement matching. Also: the fallback runs only when the capture file never resolves; if a capture file is present, the existing host-namespace PID takes precedence. The fallback target must also be tagged `host_namespace: true` so the downstream SIGTERM path uses `flatpak-spawn --host kill`.
- **VALIDATE**: `cargo test -p crosshook-core launch::watchdog:: -- --nocapture`. New tests: synthetic `children_map` with a matching/non-matching `comm`; pure-function `resolve_watchdog_target_by_exe_name` coverage; tracing subscriber capture test (using `tracing-test` if available in dev-deps, else a manual layer) asserting the `fallback` field value per branch. If `tracing-test` is not present, assert via `tracing::subscriber::with_default` + a local recorder.

### Task 2.3: Extend settings IPC merge for `steam_deck_caveats_dismissed_at` — Depends on [1.2]

- **BATCH**: B2
- **ACTION**: In `src-tauri/src/commands/settings.rs`, add `steam_deck_caveats_dismissed_at: Option<Option<String>>` to the IPC request DTO and mirror the absent-preserve / set / explicit-null-clear merge in `save_settings` (or whichever fn owns the merge).
- **IMPLEMENT**: Follow the `install_nag_dismissed_at` precedent 1:1. Add triple-state merge line + preserve/set/clear tests + JSON containment serialization test.
- **MIRROR**: `TEST_STRUCTURE — settings triple-state merge`.
- **IMPORTS**: none new.
- **GOTCHA**: Forgetting `#[serde(default, skip_serializing_if = "Option::is_none")]` on the request field breaks absent-preserve semantics (the field serializes as `null` and clears instead of preserving). Match the `install_nag_dismissed_at` field attributes exactly.
- **VALIDATE**: `cargo test -p crosshook-native commands::settings:: -- --nocapture`. Add three merge tests + JSON containment.

### Task 2.4: Add `dismiss_steam_deck_caveats` command + apply-dismissal wiring — Depends on [1.2, 1.3]

- **BATCH**: B2
- **ACTION**: In `src-tauri/src/commands/onboarding.rs`, add `#[tauri::command] pub fn dismiss_steam_deck_caveats(store: State<'_, SettingsStore>) -> Result<(), String>` that writes `steam_deck_caveats_dismissed_at = Some(Utc::now().to_rfc3339())`. In `check_readiness`, call `apply_steam_deck_caveats_dismissal(&mut result, &settings.steam_deck_caveats_dismissed_at)` alongside the existing `apply_install_nag_dismissal` call.
- **IMPLEMENT**: 8-line copy of `dismiss_umu_install_nag`, different setting field + apply helper. Tests: function-pointer signature freeze for `dismiss_steam_deck_caveats`, store-mutation test (use `SettingsStore::with_base_path(tempdir)` pattern), behavior test for `apply_steam_deck_caveats_dismissal` clearing the payload on subsequent `check_readiness`.
- **MIRROR**: `ERROR_HANDLING — Tauri command result contract`, `TEST_STRUCTURE — Tauri command signature freeze`, existing `dismiss_umu_install_nag` at `commands/onboarding.rs:28-36`.
- **IMPORTS**: `use chrono::Utc;` (already imported in this file).
- **GOTCHA**: The command must be registered in `tauri::Builder::invoke_handler` — that happens in Task 4.3 to avoid a same-file conflict with another parallel task if any. If registration is missing, IPC calls silently fail at runtime with "command X not found".
- **VALIDATE**: `cargo test -p crosshook-native commands::onboarding:: -- --nocapture`. All new + existing tests pass.

### Task 2.5: Extend browser-mode mocks for Steam-Deck caveats — Depends on [1.3, 1.4, 1.5]

- **BATCH**: B2
- **ACTION**: In `src/lib/mocks/handlers/onboarding.ts`, add a `map.set('dismiss_steam_deck_caveats', ...)` handler and extend the `check_readiness` mock to emit a `steam_deck_caveats` payload when a dev toggle is set.
- **IMPLEMENT**: Add a new toggle key `showSteamDeckCaveats` reachable via URL `?steamDeckCaveats=show` (mirror existing `?onboarding=show` wiring in `lib/toggles.ts`). In the `check_readiness` mock, include `steam_deck_caveats: (toggles.showSteamDeckCaveats && !store.settings.steam_deck_caveats_dismissed_at) ? { description: "...", items: [...], docs_url: "..." } : null`.
- **MIRROR**: `SERVICE_PATTERN — browser-mode mock handler`.
- **IMPORTS**: `getActiveToggles` from `../toggles` — already imported.
- **GOTCHA**: The `verify:no-mocks` CI sentinel ensures no prod code imports from `lib/mocks/`. Keep the new handler strictly inside the mocks tree. Also: when the caveats are rendered, the optimistic dismiss in the hook will flip the payload to `null` locally; the mock just needs to clear it on subsequent `check_readiness` calls by reading `store.settings.steam_deck_caveats_dismissed_at`.
- **VALIDATE**: `./scripts/dev-native.sh --browser` (no Rust toolchain) + visit `?steamDeckCaveats=show` and verify caveat section renders, dismiss clears it, reload keeps it dismissed.

### Task 3.1: Expose `steamDeckCaveats` + dismiss handler in `useOnboarding.ts` — Depends on [2.4, 1.4]

- **BATCH**: B3
- **ACTION**: Extend the hook to derive `steamDeckCaveats = readinessResult?.steam_deck_caveats ?? null` and add `dismissSteamDeckCaveats` that calls `invoke('dismiss_steam_deck_caveats')`, catches errors into `setCheckError`, and optimistically patches `readinessResult` to `null` out `steam_deck_caveats`.
- **IMPLEMENT**: Parallel structure to `dismissUmuInstallNag` at `useOnboarding.ts:167-175`. Export both as part of the hook's return value.
- **MIRROR**: existing `dismissUmuInstallNag` in `src/hooks/useOnboarding.ts:167-175`.
- **IMPORTS**: none new; `callCommand` + `SteamDeckCaveats` type from Task 1.4.
- **GOTCHA**: The hook's return tuple/object is consumed by multiple components. Keep the return shape additive (append new fields, do not reorder) to avoid churn in unrelated callers.
- **VALIDATE**: `./scripts/lint.sh` + `cargo test -p crosshook-native commands::onboarding::` (integration still passes). Browser-mode reload verifies the hook wires through.

### Task 4.1: Render Steam-Deck caveats `<section>` in `WizardReviewSummary.tsx` — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Add a new sibling `<section className="crosshook-onboarding-wizard__steam-deck-caveats">` below the UMU guidance section. Show only when `steamDeckCaveats != null`. Render `description`, `items` as a `<ul>` with three `<li>`s, and two buttons: "Open docs" (opens `docs_url` via `openUrl`) and "Dismiss" (calls `onDismissSteamDeckCaveats`).
- **IMPLEMENT**: Mirror the BEM + button BEM classes from the UMU section (`crosshook-button--secondary/--sm/--ghost`). Add `aria-label="Steam Deck gaming-mode caveats"`. Keep the optional chain fallback `?? null` as the gate.
- **MIRROR**: existing UMU guidance section at `WizardReviewSummary.tsx:146-185`.
- **IMPORTS**: `openUrl` from `@/lib/plugin-stubs/shell` already imported.
- **GOTCHA**: If this section becomes `overflow-y: auto`, it MUST be added to `SCROLLABLE` in `src/hooks/useScrollEnhance.ts` per CLAUDE.md — but the review summary renders inside the already-scroll-enhanced wizard body, so a non-scrolling inline section is preferred. Keep it fixed-height.
- **VALIDATE**: `./scripts/dev-native.sh --browser?steamDeckCaveats=show` — verify the section renders, docs open in host browser, dismiss triggers the hook path.

### Task 4.2: Prop-drill `steamDeckCaveats` + handler in `OnboardingWizard.tsx` — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Pull `steamDeckCaveats` and `dismissSteamDeckCaveats` from `useOnboarding()`, pass them to `<WizardReviewSummary>` via `steamDeckCaveats={...}` and `onDismissSteamDeckCaveats={...}`.
- **IMPLEMENT**: 4-line addition alongside the existing `umuInstallGuidance` / `onDismissUmuInstallNag` props.
- **MIRROR**: existing `onDismissUmuInstallNag` prop drill at `OnboardingWizard.tsx:469-475`.
- **IMPORTS**: none new.
- **GOTCHA**: Do not propagate the async dismiss function directly — wrap as `() => void dismissSteamDeckCaveats()` to satisfy React strict-mode expectations (matches existing pattern at line 474).
- **VALIDATE**: `./scripts/lint.sh` + browser smoke.

### Task 4.3: Register `dismiss_steam_deck_caveats` in `src-tauri/src/lib.rs` — Depends on [2.4]

- **BATCH**: B4
- **ACTION**: Add `commands::onboarding::dismiss_steam_deck_caveats` to the `tauri::Builder::invoke_handler(tauri::generate_handler![...])` macro call.
- **IMPLEMENT**: Single-line addition. Keep alphabetised order relative to existing onboarding commands for diff readability.
- **MIRROR**: `src-tauri/src/lib.rs:393-396` registration block.
- **IMPORTS**: none new.
- **GOTCHA**: Missing this step is the #1 "silent IPC failure" — frontend hook resolves but every invoke rejects with `command 'dismiss_steam_deck_caveats' not found`. Add a Tauri command integration-test assertion for the generated handler list if one exists, otherwise rely on manual browser smoke.
- **VALIDATE**: `cargo check -p crosshook-native` + manual browser smoke in `./scripts/dev-native.sh` (not `--browser`, which bypasses IPC).

### Task 5.1: Update PRD Decisions + Open Questions + close GitHub issues — Depends on [4.1, 4.2, 4.3, 2.2]

- **BATCH**: B5
- **ACTION**: Update `docs/prps/prds/umu-launcher-migration.prd.md`: mark Open Questions #1 (Flathub status — NOT published, resolved by this Phase 5b), #3 (gamescope SIGTERM — fallback landed, telemetry tracing added), #4 (Steam Deck gaming-mode caveats — onboarding surface landed) as `[x] RESOLVED (2026-04-XX, Phase 5b #N)` with a one-line summary. Add a Decisions Log row for "Flathub one-click install — skipped, NOT published". Add a row in the phase-tracking issues table for Phase 5b with links to #242/#244/#245 and the new tracking issue. After merge, close #242, #244, #245 with summary comments referencing the PR.
- **IMPLEMENT**: Documentation-only edit. Do NOT reopen Phase 5's `pending → complete` transition; just append Phase 5b as its own row in the Implementation Phases table.
- **MIRROR**: existing Open Questions resolution pattern (e.g. `#243` resolution note at PRD §Open Questions).
- **IMPORTS**: n/a.
- **GOTCHA**: Per CLAUDE.md, this docs-only commit MUST use a `docs(internal): …` prefix since PRDs live under `docs/prps/`. The GitHub issue closures are separate gh-CLI operations — do them after the PR lands, not in the same commit.
- **VALIDATE**: `./scripts/lint.sh` (Prettier + markdown format) + a manual PRD diff review. `gh issue view 242 / 244 / 245` after close to confirm labels + comment.

---

## Testing Strategy

### Unit Tests

| Test                                                                     | Input                                                                         | Expected Output                                                                            | Edge Case?                               |
| ------------------------------------------------------------------------ | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | ---------------------------------------- |
| `platform::is_steam_deck_from_sources` — env only                        | env `SteamDeck=1`, os_release `None`                                          | `true`                                                                                     | No                                       |
| `platform::is_steam_deck_from_sources` — os-release ID=steamos           | env empty, os_release `Some("ID=steamos\n")`                                  | `true`                                                                                     | No                                       |
| `platform::is_steam_deck_from_sources` — os-release VARIANT_ID=steamdeck | env empty, os_release `Some("ID=arch\nVARIANT_ID=steamdeck\n")`               | `true`                                                                                     | Yes (non-SteamOS distro w/ deck variant) |
| `platform::is_steam_deck_from_sources` — neither signal                  | env empty, os_release `Some("ID=arch\n")`                                     | `false`                                                                                    | No                                       |
| `settings::backward_compat_without_steam_deck_caveats_dismissed_at`      | TOML without the new field                                                    | `loaded.steam_deck_caveats_dismissed_at == None`                                           | Yes                                      |
| `settings::roundtrip_steam_deck_caveats_dismissed_at`                    | Save `Some("2026-04-15T12:00:00Z".into())`, reload                            | Same value                                                                                 | No                                       |
| `readiness::evaluate_checks_inner` — deck false, umu absent, flatpak     | is_steam_deck=false, existing case                                            | `steam_deck_caveats == None` (no regression)                                               | No                                       |
| `readiness::evaluate_checks_inner` — deck true, umu present              | is*steam_deck=true, umu_path=Some(*)                                          | `steam_deck_caveats == Some(..)`, `all_passed == true`                                     | Yes                                      |
| `readiness::evaluate_checks_inner` — deck true + dismissed               | is_steam_deck=true + `apply_steam_deck_caveats_dismissal` with Some timestamp | `steam_deck_caveats == None`                                                               | Yes                                      |
| `readiness::build_umu_install_advice` — Faugus trust-vehicle line        | Any distro family                                                             | `guidance.description` contains `"Faugus Launcher"` + `"io.github.Faugus.faugus-launcher"` | No                                       |
| `watchdog::resolve_watchdog_target_by_exe_name` — match                  | children_map with exe_name match                                              | `Some(ShutdownTarget { pid, host_namespace: true })`                                       | No                                       |
| `watchdog::resolve_watchdog_target_by_exe_name` — no match               | children_map without match                                                    | `None`                                                                                     | Yes                                      |
| `watchdog::resolve_watchdog_target` — capture-file branch                | capture file resolves                                                         | Returns capture target; tracing field `fallback = "capture_file"`                          | No                                       |
| `watchdog::resolve_watchdog_target` — exe-name fallback                  | capture file never resolves, exe found                                        | Returns exe-fallback target; tracing field `fallback = "exe_fallback"`                     | Yes                                      |
| `watchdog::resolve_watchdog_target` — none branch                        | capture file never resolves, exe not found                                    | Returns `None`; tracing field `fallback = "none"`                                          | Yes                                      |
| `commands::settings::merge_steam_deck_caveats_dismissed_at`              | request `Some(Some(ts))`                                                      | merged field equals `Some(ts)`                                                             | No                                       |
| `commands::settings::clear_steam_deck_caveats_dismissed_at`              | request `Some(None)`                                                          | merged field `None`                                                                        | Yes (explicit null)                      |
| `commands::settings::preserve_steam_deck_caveats_dismissed_at`           | request `None` (absent field)                                                 | merged field preserved from current                                                        | Yes (absent)                             |
| `commands::onboarding::dismiss_steam_deck_caveats` signature freeze      | `fn(State<'_, SettingsStore>) -> Result<(), String>`                          | Compiles                                                                                   | No                                       |
| `commands::onboarding::dismiss_steam_deck_caveats` behavior              | Fresh store + call                                                            | Settings TOML gains RFC3339 timestamp                                                      | No                                       |
| `commands::onboarding::check_readiness` — applies caveats dismissal      | Pre-dismissed settings + readiness probe                                      | `result.steam_deck_caveats == None`                                                        | No                                       |

### Edge Cases Checklist

- [ ] Fresh install (no settings file) — new field defaults to `None`, caveats render on Deck.
- [ ] Existing install with `install_nag_dismissed_at` set — backward compat: new field absent → defaults.
- [ ] Deck in desktop mode (`is_steam_deck == true`, `is_inside_gamescope_session == false`) — caveats still render.
- [ ] Deck in gaming mode (`is_inside_gamescope_session == true`) — caveats still render; no conflict with umu guidance.
- [ ] Non-Deck Flatpak user — caveats do NOT render; umu guidance unchanged.
- [ ] Non-Deck native user — both caveats + umu guidance may render independently.
- [ ] Watchdog: gamescope never started (no PID) — watchdog early-aborts as today; fallback does not trigger.
- [ ] Watchdog: capture file written but game process exited before watchdog woke — existing descendant-cleanup logic handles; no fallback needed.
- [ ] Watchdog: `TASK_COMM_LEN=15` truncation for long exe names — `cmdline` fallback inside `is_host_descendant_process_running` covers.
- [ ] Concurrent `check_readiness` invocations — `SettingsStore::update` already serialises via `io_lock`; no new race.
- [ ] User dismisses caveats, then restores OS, reloads CrossHook — caveats stay dismissed (intentional; PRD open question §dismissal-unsnooze is OUT OF SCOPE here).
- [ ] Flathub URL later added for `org.openwinecomponents.umu.umu-launcher` — follow-up issue re-opens #242; Phase 5b does not pre-wire conditional UI for it.

---

## Validation Commands

### Static Analysis

```bash
./scripts/lint.sh
```

EXPECT: Zero lint errors across Rust (`cargo fmt` + `cargo clippy -D warnings`), TypeScript (Biome), Markdown (Prettier), Shell (ShellCheck).

### Unit Tests — Rust core

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: All existing tests pass + new tests for `platform::is_steam_deck_from_sources`, `settings::backward_compat_without_steam_deck_caveats_dismissed_at`, `readiness::evaluate_checks_inner` matrix, `readiness::build_umu_install_advice` Faugus substring, `launch::watchdog::resolve_watchdog_target_by_exe_name`, and fallback-outcome tracing.

### Unit Tests — Tauri layer

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native commands::
```

EXPECT: New triple-state merge + signature-freeze + behavior tests for `dismiss_steam_deck_caveats` + `check_readiness` apply-dismissal pass.

### Full Test Suite

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml
```

EXPECT: No regressions across both crates.

### Browser Validation

```bash
./scripts/dev-native.sh --browser
```

Then visit `http://127.0.0.1:1420/?onboarding=show&steamDeckCaveats=show`. EXPECT: Steam-Deck caveats `<section>` renders below UMU guidance; "Open docs" opens the docs_url; "Dismiss" clears the section and survives reload.

### Build (Native)

```bash
cargo check --manifest-path src/crosshook-native/Cargo.toml
```

EXPECT: Zero compile errors across both crates.

### Manual Validation

- [ ] On a non-Deck native host: confirm caveats section does NOT render.
- [ ] On a Deck in desktop mode (if available): confirm caveats render with all three bullets.
- [ ] On a Deck in gaming mode: confirm caveats render and do not overlap umu guidance.
- [ ] On a Flatpak host with no `umu-run`: confirm description now includes the Faugus line.
- [ ] Flatpak teardown scenario: `flatpak run dev.crosshook.CrossHook` → launch under gamescope → kill gamescope from outside → confirm CrossHook log shows `fallback = "exe_fallback"` or `"capture_file"` (not `"none"`) and the Wine game PID no longer survives.
- [ ] Dismiss caveats → reload → caveats stay dismissed.

---

## Acceptance Criteria

- [ ] `is_steam_deck()` is available on `crosshook_core::platform` and covered by closure-injected tests.
- [ ] `ReadinessCheckResult.steam_deck_caveats` is populated on Deck and cleared on dismissal.
- [ ] `dismiss_steam_deck_caveats` Tauri command is registered in `invoke_handler` and returns `Result<(), String>`.
- [ ] Settings IPC merges `steam_deck_caveats_dismissed_at` via triple-state semantics with three passing tests.
- [ ] Browser-mode mocks expose the new handler gated by a dev toggle.
- [ ] `WizardReviewSummary` renders a new `<section>` when caveats are present; no layout regression for the existing umu guidance.
- [ ] Watchdog stand-down fallback reliably locates the Wine game PID in host PID space by exe name; tracing emits `fallback = "exe_fallback" | "capture_file" | "none"`.
- [ ] `build_umu_install_advice` description now includes a Faugus trust-vehicle line pointing at `io.github.Faugus.faugus-launcher`.
- [ ] PRD Open Questions #1, #3, #4 marked resolved with Phase 5b references.
- [ ] `./scripts/lint.sh` passes; `cargo test -p crosshook-core` passes; `cargo test -p crosshook-native` passes.
- [ ] No type errors in TS (`biome check`).
- [ ] Matches UX design above for the review summary.

## Completion Checklist

- [ ] Code follows discovered patterns (RFC3339 dismissal, atomic settings update, snake_case IPC, BEM CSS).
- [ ] Error handling matches codebase style (`Result<T, String>` at IPC boundary, `anyhow` / mapped errors below).
- [ ] Logging follows codebase conventions (structured `tracing` fields, no `target=` scoping).
- [ ] Tests follow test patterns (closure-injected pure helpers, function-pointer signature freezes, triple-state merge assertions, pure-map watchdog unit tests).
- [ ] No hardcoded user home paths in Rust; Faugus probe uses `env::var("HOME")` + `Path::new`.
- [ ] Documentation updated: PRD Open Questions + Decisions Log + Implementation Phases table.
- [ ] No unnecessary scope additions (no new supervisor, no Flathub UI, no unsnooze, no auto-workarounds).
- [ ] Self-contained — no questions needed during implementation.
- [ ] GitHub issues #242, #244, #245 closed with summary comments referencing the merged PR.
- [ ] Browser-mode `verify:no-mocks` CI sentinel still clean.

## Risks

| Risk                                                                                                                    | Likelihood | Impact | Mitigation                                                                                                                                                                                           |
| ----------------------------------------------------------------------------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exe-name fallback matches wrong descendant (e.g. a helper with the same `comm` as the game exe) and SIGKILLs it         | M          | H      | Require exe-name match AND ancestry under observed_gamescope_pid; log both `discovered_pid` and `match_source = "comm" \| "cmdline"`; ship behind same tracing so field reports pinpoint mismatches. |
| `TASK_COMM_LEN=15` truncates a long exe name so the `comm` match fails                                                  | M          | M      | Reuse `is_host_descendant_process_running` which already falls back to `/proc/<pid>/cmdline` — new code must not bypass that helper.                                                                 |
| Faugus Launcher Flathub app ID changes upstream                                                                         | L          | L      | The ID is only referenced in two places (description string + candidate probe); a single constant keeps renames cheap. Document the constant in `runtime_helpers.rs`.                                |
| Flathub later publishes `org.openwinecomponents.umu.umu-launcher`                                                       | L          | M      | Phase 5b intentionally does NOT pre-wire a conditional one-click UI. Follow-up issue re-opens #242 if/when published — cheap incremental add.                                                        |
| SteamOS detection triggers on non-Deck systems that ship `ID=steamos` (e.g. HoloISO) and shows irrelevant caveats       | L          | L      | Caveat text explicitly says "SteamOS / Steam Deck gaming mode"; HoloISO users will understand. If noise reports come in, tighten detection to require both `ID=steamos` AND `VARIANT_ID=steamdeck`.  |
| Adding `is_steam_deck: bool` to `evaluate_checks_inner` breaks callers in `crosshook-cli` or other crates               | L          | L      | Default the top-level `evaluate_checks` to call `is_steam_deck()` for callers; only the test-injection `evaluate_checks_inner` takes the bool.                                                       |
| Flatpak manual teardown smoke test needs a real Flatpak build; dev host may not provide one                             | M          | L      | Gate the Flatpak manual check to the native-build validation list; keep the unit-test coverage strong enough that merging without a live Flatpak smoke is acceptable.                                |
| Caveats block the review-step scroll on small windows                                                                   | L          | L      | Keep the section non-scrollable (fixed height) and under the review summary scroll container; SCROLLABLE registry unchanged.                                                                         |
| Launch-log parser (`diagnostic_method_for_log`) breaks if the new watchdog tracing accidentally leaks to the launch log | L          | M      | Emit via `tracing::` (not `println!`); the launch log stream captures child stdout/stderr only, not the Rust tracing layer.                                                                          |
| Browser-mode toggle naming collision                                                                                    | L          | L      | Namespace the toggle as `steamDeckCaveats` (camelCase) per existing convention in `lib/toggles.ts`; ensure no collision with existing keys.                                                          |

## Notes

- **Flathub re-check (Open Question §1)**: Verified 2026-04-15 at plan time — `https://flathub.org/apps/org.openwinecomponents.umu.umu-launcher` returns 404 and the appstream API returns 404. This is the authoritative negative signal that closes #242. Faugus Launcher `io.github.Faugus.faugus-launcher` IS published and bundles umu-launcher 1.4+; it becomes the documented trust vehicle.
- **Watchdog fallback philosophy (#244)**: The existing watchdog only acts when the gamescope PID capture file resolves. The stand-down path was invisible — if pressure-vessel ever held onto Wine processes after gamescope died, CrossHook would not know. Phase 5b makes the outcome observable (three-way `fallback` tracing field) AND adds a host-ps exe-name fallback so real teardown still happens. If field reports later show exe-name matching is unreliable, the structured tracing already scaffolds the decision to add a second pressure-vessel-side capture script — no PRD rewrite needed.
- **Caveats content is intentionally terse (#245)**: Three bullets, one docs URL, one dismiss. The PRD explicitly scopes this as "document, not solve". Phase 5b keeps the copy hard-coded English — internationalisation + recurring reminders + per-release re-surface logic are deferred to future issues.
- **Dependencies**: No new crate deps. `nix = "0.31.2"` feature expansion to `["signal", "process"]` is NOT required by this plan — the watchdog fallback reuses the existing host-command primitives. If a future issue needs in-sandbox signaling, that's a separate Cargo.toml edit.
- **Consistency with CLAUDE.md**: Every new TOML field has a `backward_compat_without_<field>` test (per CLAUDE.md persistence rule); the PRD + report files stay under `docs/prps/` and take `docs(internal): …` commit prefix; no ad-hoc issue labels introduced.
- **Follow-ups (out of scope for Phase 5b)**: (1) Recurring caveats re-surface after N days or release bump, (2) in-container Wine-PID capture script if exe-name matching proves insufficient, (3) one-click Flathub install for umu-launcher if upstream publishes, (4) telemetry baseline for launch-outcome tracking (PRD §Open Questions §5).
