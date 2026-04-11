# Plan: Flatpak Phase 3 Process Execution Hardening

## Summary

Phase 3 makes the existing Flatpak bundle behave like the native app for host-dependent workflows. The work finishes host-command migration, preserves Proton/Wine env propagation across `flatpak-spawn --host`, makes system Proton discovery and validation host-aware, hardens helper scripts, and turns the `unshare` limitation into a persistent warning instead of a launch-time surprise.

## User Story

As an immutable-distro or SteamOS user running CrossHook as a Flatpak, I want host binaries, Proton discovery, trainer helpers, and network-isolation warnings to behave predictably inside the sandbox, so that Flatpak launches are functionally equivalent to the native build except where the sandbox imposes a clear, visible limitation.

## Problem → Solution

Flatpak hides host `/usr` and host binaries, but several CrossHook paths still assume direct `Command::new(...)`, direct shell execution, or sandbox PATH checks are valid. The fix is a single host-execution layer for async and sync callers, env-aware launch builders, helper-script hardening, and a clear UI signal for the one intentional degradation: blocked `unshare --user --net`.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/flatpak-distribution.prd.md`
- **PRD Phase**: Phase 3 — Process Execution Hardening
- **Source Issue**: `#209`
- **Estimated Files**: 17

## Persistence & Usability

### Storage Boundary

| Datum / behavior                                                               | Classification | Notes                                                                                                       |
| ------------------------------------------------------------------------------ | -------------- | ----------------------------------------------------------------------------------------------------------- |
| Host command wrappers, host directory probes, host command availability checks | Runtime-only   | No new persisted state; all decisions derive from `FLATPAK_ID`, `/.flatpak-info`, and live command results. |
| Flatpak/unshare capability status returned to the frontend                     | Runtime-only   | Read-only IPC payload used to drive badges and warnings; no TOML or SQLite storage.                         |
| Profile warning badge for unavailable network isolation                        | Runtime-only   | Derived from profile launch settings plus live platform capabilities; not stored back into the profile.     |

### Persistence & Usability Notes

- No TOML migration is planned. Profile settings stay unchanged; the new warning state is derived at runtime.
- No SQLite migration is planned. Phase 3 does not add launch history or metadata schema.
- Offline behavior is unchanged. The badge only reflects a platform constraint.
- When `unshare --user --net` is unavailable, launch still proceeds and the UI keeps a visible warning on affected profiles.
- Users can toggle network isolation in the profile, but cannot dismiss the capability badge while the constraint remains true.

---

## UX Design

### Before

- Flatpak builds can start, but several host-dependent flows fail or mis-detect binaries because CrossHook still probes sandbox PATHs or sandbox `/usr`.
- `validate_launch` can warn about `unshare`, but the affected state is not surfaced as a persistent profile-level badge.
- Helper scripts assume host commands such as `steam`, `pgrep`, `proton`, and `gamescope` are directly reachable.

### After

- Host-dependent backend flows route through a single platform layer that works for async launch builders, sync utility code, and host-only directory probes.
- Profiles that request network isolation inside a Flatpak environment with blocked `unshare` show a persistent warning chip in selector/detail surfaces and still launch safely without network isolation.
- Helper-script logs and runtime behavior stay aligned with the Rust launch layer, so Flatpak troubleshooting uses the same command story everywhere.

### Interaction Changes

| Touchpoint                   | Before                                                            | After                                                                                            | Notes                                                             |
| ---------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| Launch validation            | `unshare` warning appears only during validation/launch           | Same warning still appears, but it is backed by host-aware probing                               | No new blocking modal or toast                                    |
| Profile selectors            | No persistent Flatpak-specific status for affected profiles       | Affected profiles show a non-dismissible badge in selectors that already support dropdown badges | Reuse `ThemedSelect.badge` instead of inventing a new list widget |
| Profile detail / hero status | Health and offline chips only                                     | Flatpak network-isolation badge joins the existing status row for affected profiles              | Tooltip explains the degraded behavior                            |
| Helper-script execution      | Direct shell invocations assume host binaries are in sandbox PATH | Host-only commands are routed through `flatpak-spawn --host` when needed                         | Keep sandbox-local file ops local                                 |

---

## Mandatory Reading

| Priority | File                                                                       | Lines                      | Why                                                 |
| -------- | -------------------------------------------------------------------------- | -------------------------- | --------------------------------------------------- |
| P0       | `docs/prps/prds/flatpak-distribution.prd.md`                               | 96-141, 425-451, 457-489   | Phase 3 requirements and storage boundary           |
| P0       | `src/crosshook-native/crates/crosshook-core/src/platform.rs`               | 15-135, 307-374            | Flatpak detection, host wrappers, env caveat, tests |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | 131-321, 1351-1525         | Env-bearing launch builders and unshare tests       |
| P0       | `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | 22-121, 333-357            | Proton/gamescope builders and unshare probe         |
| P0       | `src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`           | 9-40, 208-264, 500-598     | System compat-tool discovery and tests              |
| P1       | `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`         | 15-18, 508-514, 564-642    | Git env handling and command execution              |
| P1       | `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | 56-89                      | Sync `getent` lookup                                |
| P1       | `src/crosshook-native/crates/crosshook-core/src/export/diagnostics.rs`     | 246-266                    | Sync `lspci` probe                                  |
| P1       | `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`   | 103-108, 253-267           | Required-binary validation                          |
| P1       | `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`         | 323-385, 519-733, 999-1037 | Stable validation codes and `unshare` warning       |
| P1       | `src/crosshook-native/runtime-helpers/steam-launch-helper.sh`              | 262-403                    | Steam/`pgrep`/Proton helper flow                    |
| P1       | `src/crosshook-native/runtime-helpers/steam-launch-trainer.sh`             | 111-165                    | Detached runner contract                            |
| P1       | `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`        | 207-288                    | Direct Proton/gamescope runner flow                 |
| P1       | `src/crosshook-native/src-tauri/src/commands/launch.rs`                    | 44-76                      | Thin IPC command style                              |
| P1       | `src/crosshook-native/src/components/ui/ThemedSelect.tsx`                  | 12-54                      | Existing selector badge support                     |
| P1       | `src/crosshook-native/src/components/pages/ProfilesPage.tsx`               | 541-614, 700-702           | Existing status-chip composition                    |
| P1       | `src/crosshook-native/src/components/LaunchSubTabs.tsx`                    | 324-365                    | Existing warning list rendering                     |
| P2       | `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`         | 417-530                    | Env collection pattern                              |

## External Documentation

No new external research is required for this plan. The Phase 3 PRD, issue `#209`, and the current `platform.rs` documentation already capture the relevant Flatpak constraints and host-execution tradeoffs that implementation must follow.

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### TAURI_COMMAND_PATTERN

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/launch.rs:73-76
#[tauri::command]
pub fn check_gamescope_session() -> bool {
    crosshook_core::launch::is_inside_gamescope_session()
}
```

### ENV_COLLECTION_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/preview.rs:504-514
for (key, value) in &directives.env {
    upsert_preview_env(env, key, value, EnvVarSource::LaunchOptimization);
}
for (key, value) in &request.custom_env_vars {
    upsert_preview_env(env, key, value, EnvVarSource::ProfileCustom);
}
```

### GIT_ENV_WRAPPER_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/community/taps.rs:508-513
let mut command = Command::new("git");
for (key, value) in git_security_env_pairs() {
    command.env(key, value);
}
command
```

### LOGGING_FIELD_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/platform.rs:109-113
tracing::debug!(program, "wrapping command with flatpak-spawn --host");
let mut cmd = Command::new("flatpak-spawn");
cmd.arg("--host").arg(program);
```

### COMMAND_WRAPPER_TEST_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/platform.rs:308-313
let cmd = host_command_with("ls", true);
let std_cmd = cmd.as_std();
assert_eq!(std_cmd.get_program(), "flatpak-spawn");
let args: Vec<&std::ffi::OsStr> = std_cmd.get_args().collect();
assert_eq!(args, vec![std::ffi::OsStr::new("--host"), std::ffi::OsStr::new("ls")]);
```

### PROTON_DISCOVERY_TEST_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/steam/proton.rs:556-561
let tools = discover_compat_tools_with_roots(
    vec![steam_root.path().to_path_buf()],
    vec![system_root.path().to_path_buf()],
    &mut diagnostics,
);
```

### SELECT_BADGE_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/ui/ThemedSelect.tsx:51-54
<Select.ItemText className="crosshook-themed-select__item-text">{opt.label}</Select.ItemText>;
{
  opt.badge ? <span className="crosshook-themed-select__item-badge">{opt.badge}</span> : null;
}
```

### STATUS_CHIP_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/ProfilesPage.tsx:556-563
<span
  className={`crosshook-status-chip crosshook-version-badge crosshook-version-badge--${isWarning ? 'warning' : 'info'}`}
  title={isWarning ? 'Version mismatch detected since last successful launch' : 'Steam is currently updating this game'}
>
```

### VALIDATION_NODE_MAPPING_PATTERN

```ts
// SOURCE: src/crosshook-native/src/utils/mapValidationToNode.ts:36-40
if (code.startsWith('steam_')) {
  return 'steam';
}
if (code.startsWith('trainer_') || code.startsWith('native_trainer') || code.startsWith('unshare_net')) {
  return 'trainer';
}
```

---

## Files to Change

| File                                                                       | Action | Justification                                                     |
| -------------------------------------------------------------------------- | ------ | ----------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/platform.rs`               | UPDATE | Add sync wrappers and host-only probes                            |
| `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`         | UPDATE | Move git calls onto host-aware env handling                       |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | UPDATE | Make `getent` host-aware                                          |
| `src/crosshook-native/crates/crosshook-core/src/export/diagnostics.rs`     | UPDATE | Make `lspci` host-aware                                           |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | UPDATE | Rebuild Proton/gamescope/unshare helpers around platform wrappers |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | UPDATE | Refactor env-bearing launch builders                              |
| `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`   | UPDATE | Make required-binary validation host-aware                        |
| `src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`           | UPDATE | Probe system compat-tool roots through the host                   |
| `src/crosshook-native/runtime-helpers/steam-launch-helper.sh`              | UPDATE | Host-wrap Steam/`pgrep`/Proton calls                              |
| `src/crosshook-native/runtime-helpers/steam-launch-trainer.sh`             | UPDATE | Keep detached runner aligned with helper changes                  |
| `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`        | UPDATE | Host-wrap Proton and gamescope                                    |
| `src/crosshook-native/src-tauri/src/commands/launch.rs`                    | UPDATE | Add capability IPC                                                |
| `src/crosshook-native/src-tauri/src/commands/run_executable.rs`            | UPDATE | Remove `/usr/bin/rm` fallback and host-wrap `kill`                |
| `src/crosshook-native/src-tauri/src/commands/update.rs`                    | UPDATE | Host-wrap `kill`                                                  |
| `src/crosshook-native/src/types/launch.ts`                                 | UPDATE | Add capability payload typing                                     |
| `src/crosshook-native/src/hooks/useLaunchPlatformStatus.ts`                | CREATE | Centralize capability IPC loading                                 |
| `src/crosshook-native/src/components/pages/LaunchPage.tsx`                 | UPDATE | Badge affected profiles in the selector                           |
| `src/crosshook-native/src/components/pages/ProfilesPage.tsx`               | UPDATE | Show persistent detail/selector warning chips                     |

## NOT Building

- New Flatpak permissions or manifest changes. Phase 3 hardens execution inside the existing sandbox, it does not reopen Phase 1 packaging scope.
- A Flathub submission workflow. That belongs to Phase 4.
- New TOML or SQLite persistence. Platform capability and badge state stay runtime-only.
- A rewrite of AppImage-only startup re-exec logic in `src-tauri/src/lib.rs`. That path is not a Flatpak process-execution regression.
- A brand-new profile list UI. Reuse the existing selector badge and status-chip surfaces instead of introducing a new browsing widget.

---

## Step-by-Step Tasks

### Task 1: Extend platform host-execution primitives

- **ACTION**: Expand `crosshook-core/src/platform.rs` so Phase 3 has one coherent host-execution surface for both async and sync callers.
- **IMPLEMENT**: Keep `host_command()` / `host_command_with_env()` for async code, add sync counterparts for `std::process::Command`, and add narrowly-scoped helpers such as host command existence checks and host directory listing for fixed system roots. Extend the existing Flatpak-vs-native tests instead of creating a second platform helper module.
- **MIRROR**: `LOGGING_FIELD_PATTERN`, `COMMAND_WRAPPER_TEST_PATTERN`
- **IMPORTS**: `std::process::Command`, `std::collections::BTreeMap`, `std::path::Path`
- **GOTCHA**: Do not introduce shell-based helpers for user-controlled strings. Any shell fallback must stay limited to fixed binary names or fixed system directories.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

### Task 2: Refactor env-bearing launch builders before replacing `Command::new(...)`

- **ACTION**: Move Proton/game/helper launch builders onto env-aware host wrappers without losing `WINEPREFIX`, `STEAM_COMPAT_*`, `MANGOHUD_*`, or custom env vars inside Flatpak.
- **IMPLEMENT**: Mirror the preview layer by collecting host/runtime/optimization/custom env into a `BTreeMap` first, then constructing the final process with `host_command_with_env(...)` rather than calling `.env()` after the command already exists. Update `launch/runtime_helpers.rs` and `launch/script_runner.rs` together so direct Proton, gamescope, wrappers, and unshare handling all stay aligned.
- **MIRROR**: `ENV_COLLECTION_PATTERN`
- **IMPORTS**: `std::collections::BTreeMap`, `crate::platform::host_command_with_env`
- **GOTCHA**: A blind search/replace to `host_command(...)` is incorrect for Flatpak because post-construction `.env()` calls are silently dropped by `flatpak-spawn --host`.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

### Task 3: Migrate sync and async host-binary call sites to the correct wrapper

- **ACTION**: Replace the remaining direct host-binary spawns in core and Tauri with the appropriate async or sync platform helper.
- **IMPLEMENT**: Update `community/taps.rs` to use env-aware host git commands, switch sync `getent`, `lspci`, and `kill` calls to the new sync wrapper, and keep AppImage-only re-exec code out of scope unless a Flatpak caller truly depends on it. Preserve current error/log wording where possible so diagnostics remain stable.
- **MIRROR**: `TAURI_COMMAND_PATTERN`, `GIT_ENV_WRAPPER_PATTERN`, `LOGGING_FIELD_PATTERN`
- **IMPORTS**: `crate::platform::{host_command_with_env, host_std_command, host_std_command_with_env}`
- **GOTCHA**: `kill` and similar sync commands live in startup/blocking code paths; do not force Tokio futures into those sites just to reuse the async wrapper.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` and `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --no-run`

### Task 4: Make validation and system Proton discovery host-aware

- **ACTION**: Fix the probes that currently look at sandbox PATH or sandbox `/usr` and therefore misreport host state.
- **IMPLEMENT**: Teach `launch/optimizations.rs` to check required binaries against the host when Flatpak is active, then update `steam/proton.rs` so system compat-tool roots are enumerated through host-visible directory helpers while user-owned library paths still use direct filesystem access. Keep diagnostics strings explicit about whether a tool came from a Steam root or a system root.
- **MIRROR**: `PROTON_DISCOVERY_TEST_PATTERN`, `VALIDATION_NODE_MAPPING_PATTERN`
- **IMPORTS**: `crate::platform::{host_command_exists, host_read_dir_names}`
- **GOTCHA**: Only fixed system roots such as `/usr/share/steam/compatibilitytools.d` should use host directory probing. User-selected paths must not be routed through a shell helper.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

### Task 5: Harden the bundled helper scripts for Flatpak host execution

- **ACTION**: Add a minimal Flatpak-aware shell wrapper and route the actual host-only commands through it in the Steam helper scripts.
- **IMPLEMENT**: In `steam-launch-helper.sh` and `steam-host-trainer-runner.sh`, add `run_host()` / equivalent and use it for `steam`, `pgrep`, direct Proton runs, and gamescope launches when `FLATPAK_ID` is present; keep sandbox-local file operations (`mkdir`, `cp`, `rm`, `realpath`) local. Touch `steam-launch-trainer.sh` only where the detached runner contract needs to stay consistent with the new wrapper behavior.
- **MIRROR**: `LOGGING_FIELD_PATTERN`
- **IMPORTS**: N/A
- **GOTCHA**: Do not `flatpak-spawn --host` the helper scripts themselves. Those scripts live inside the app image and are not host files.
- **VALIDATE**: `./scripts/build-flatpak.sh --strict` followed by helper-log inspection during manual Flatpak smoke tests

### Task 6: Remove `/usr/bin/rm` fallback and keep stop/cancel flows host-safe

- **ACTION**: Finish the standalone cleanup issue and make sure force-stop paths still work against host-spawned children.
- **IMPLEMENT**: Replace the final `/usr/bin/rm -rf` fallback in `run_executable.rs` with a `remove_dir_all` retry strategy that preserves the existing canonical-prefix guard, then route `kill` calls in `run_executable.rs` and `update.rs` through the sync host wrapper. Keep the current `/proc` sweep semantics intact.
- **MIRROR**: `LOGGING_FIELD_PATTERN`
- **IMPORTS**: `std::fs`, `crosshook_core::platform::host_std_command`
- **GOTCHA**: Removing `rm -rf` must not loosen the `_run-adhoc` namespace safety check; the deletion boundary stays exactly as strict as it is now.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

### Task 7: Surface the Flatpak `unshare` degradation as a persistent profile badge

- **ACTION**: Expose a small runtime capability contract and reuse existing selector/status-chip UI to show which profiles cannot get network isolation inside Flatpak.
- **IMPLEMENT**: Add a thin launch-platform IPC command in `src-tauri/src/commands/launch.rs`, create `useLaunchPlatformStatus.ts`, extend the launch types with the new payload, and use `ThemedSelect.badge` plus the existing Profiles/Launch status rows to display a non-dismissible tooltip-backed warning whenever `is_flatpak && !unshare_net_available && profile.launch.network_isolation` is true. Keep launch-time warnings flowing through the existing `validate_launch` + `LaunchSubTabs` pathway.
- **MIRROR**: `TAURI_COMMAND_PATTERN`, `SELECT_BADGE_PATTERN`, `STATUS_CHIP_PATTERN`, `VALIDATION_NODE_MAPPING_PATTERN`
- **IMPORTS**: `callCommand`, new `LaunchPlatformStatus` type, existing status-chip classes
- **GOTCHA**: Do not derive the persistent badge by running full launch validation for every dropdown option. Use one global capability fetch plus per-profile launch settings.
- **VALIDATE**: `npm exec --yes tsc -- --noEmit` in `src/crosshook-native`

### Task 8: Prove the full Phase 3 matrix in a real Flatpak install

- **ACTION**: Re-verify the hardening work inside the sandbox instead of treating native or browser-only checks as sufficient.
- **IMPLEMENT**: Build and install the Flatpak bundle, then execute the issue-`#209` gate matrix: system Proton discovery, Steam launch from home and external drives, helper-script `pgrep`, community tap git sync, GE-Proton download, `unshare` fallback badge/warning, gamescope wrapping, and `lspci` diagnostics. Record which checks are automated and which remain manual.
- **MIRROR**: Issue `#209` execution order and phase-gate wording
- **IMPORTS**: N/A
- **GOTCHA**: Browser dev mode and AppImage verification do not exercise host execution through `flatpak-spawn --host`; they are not substitutes for this phase gate.
- **VALIDATE**: Run the command block in `## Validation Commands` and complete the manual checklist below

---

## Testing Strategy

### Unit Tests

| Test                            | Input                                                  | Expected Output                                                                     | Edge Case? |
| ------------------------------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------- | ---------- |
| Platform wrapper tests          | Flatpak=true / Flatpak=false host command construction | Async and sync wrappers emit `flatpak-spawn --host` only inside Flatpak             | Yes        |
| Env propagation tests           | Proton/Wine env map with Flatpak host wrapper          | Required env vars become `--env=...` args instead of being dropped                  | Yes        |
| Proton discovery tests          | Steam root + synthetic system compat root              | Discovery includes official, custom, and system tools with stable diagnostics       | Yes        |
| Script-runner command tests     | `network_isolation=true/false`, gamescope on/off       | Command assembly preserves existing semantics while switching to host-safe wrappers | Yes        |
| Frontend selector/status typing | Capability payload + affected/non-affected profiles    | Badge rendering compiles cleanly and only flags the intended profiles               | Yes        |

### Edge Cases Checklist

- [ ] Flatpak active, host command exists, and env-bearing command still receives `WINEPREFIX` / `STEAM_COMPAT_*`
- [ ] Flatpak active, host binary is missing, and validation returns the existing warning/fatal surface instead of a crash
- [ ] System Proton is installed only under host `/usr/share/steam/compatibilitytools.d`
- [ ] External Steam library is mounted under `/mnt` or `/run/media`
- [ ] `network_isolation=true` on a profile while `unshare --user --net` is blocked
- [ ] Profile does not request network isolation and therefore should not receive the Flatpak badge
- [ ] Helper scripts can launch trainers whose paths contain spaces
- [ ] Stop/cancel/update flows still terminate host-spawned processes from inside Flatpak

## Validation Commands

### Backend Tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

EXPECT: `platform`, launch, Proton discovery, and helper-related unit tests all pass.

### Native Backend Compile Check

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --no-run
```

EXPECT: Tauri command-layer changes compile cleanly.

### Frontend Type Check

```bash
cd src/crosshook-native
npm exec --yes tsc -- --noEmit
```

EXPECT: Zero TypeScript errors after adding the capability hook and badge wiring.

### Flatpak Build Validation

```bash
./scripts/build-flatpak.sh --strict
```

EXPECT: The bundle builds successfully with the current manifest and helper scripts.

### Manual Validation

- [ ] `flatpak install --user --reinstall dist/CrossHook_*.flatpak`
- [ ] `flatpak run dev.crosshook.CrossHook`
- [ ] T5.1: system Proton discovery resolves a host-installed compat tool such as `proton-cachyos-slr`
- [ ] T6: Steam game + trainer launch works when the library lives under `$HOME`
- [ ] T7: Steam game + trainer launch works when the library lives under `/mnt/...` or `/run/media/...`
- [ ] T8: Helper-script process detection works through host `pgrep`
- [ ] T10: GE-Proton download/extract still works
- [ ] T11: Community tap clone/fetch works through host `git`
- [ ] T12: A profile with network isolation enabled shows the Flatpak badge and still launches without `unshare`
- [ ] T16: Gamescope wrapper launches through the host binary
- [ ] T17: Diagnostics capture GPU info through host `lspci`

## Acceptance Criteria

- [ ] Phase 3 uses one coherent host-execution abstraction for async and sync call sites
- [ ] Env-bearing Proton/game/helper launch builders preserve required env vars inside Flatpak
- [ ] System Proton discovery and required-binary validation read host state instead of sandbox state
- [ ] Bundled helper scripts route host-only commands through Flatpak-aware wrappers
- [ ] `/usr/bin/rm` fallback is removed and stop/cancel/update flows remain functional
- [ ] Profiles affected by blocked `unshare` show a persistent warning badge with explanatory copy
- [ ] The issue-`#209` manual gate matrix passes in a real Flatpak install

## Completion Checklist

- [ ] Code follows the existing `platform.rs` wrapper/test conventions instead of introducing a second platform helper surface
- [ ] Host-execution logging uses structured `tracing` fields for program/path context
- [ ] Validation/badge code reuses stable `LaunchValidationIssue.code` semantics
- [ ] Selector/detail UI reuses existing badge and status-chip patterns
- [ ] No new persistence was added to TOML or SQLite
- [ ] Helper scripts only host-wrap real host process launches, not sandbox-local file manipulation
- [ ] Manual Flatpak verification results are captured before closing Phase 3
- [ ] Scope stays within issue `#209` and its child issues `#201`–`#205`

## Risks

| Risk                                                                                          | Likelihood | Impact | Mitigation                                                                                                                        |
| --------------------------------------------------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------------- |
| Env-bearing host commands silently lose `WINEPREFIX` / `STEAM_COMPAT_*` after migration       | Medium     | High   | Refactor env collection before call-site replacement and add explicit Flatpak env-wrapper tests                                   |
| Host command availability checks regress non-Flatpak behavior or become shell-injection prone | Medium     | High   | Keep helpers dual-mode, limit any shell usage to fixed binary names, and cover both modes in unit tests                           |
| Helper-script host wrapping diverges from Rust launch behavior                                | Medium     | Medium | Reuse the existing helper structure, keep wrapper logic minimal, and verify with real Flatpak log output                          |
| Persistent `unshare` badge becomes noisy on unaffected profiles                               | Low        | Medium | Derive it from one capability fetch plus `profile.launch.network_isolation`, not from broad validation failures                   |
| System Proton discovery still misses distro-specific install roots                            | Medium     | Medium | Preserve current system root list, log which roots were probed, and validate against at least one real host-installed compat tool |

## Notes

- Issue `#209` already provides the dependency order for this phase: `#201` enables `#203`, `#204`, and `#205`, while `#202` can land independently.
- `platform.rs` already documents the main pitfall: Flatpak host wrappers must receive env vars at construction time.
- Existing launch/profile UI already has the right primitives (`ThemedSelect.badge`, status chips, `launchPathWarnings`). Reuse them instead of adding a new notification system.
