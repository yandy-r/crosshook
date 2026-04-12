# Implementation Report: Flatpak Phase 3 Process Execution Hardening

## Summary

Phase 3 hardening is implemented: sync/async host wrappers in `platform.rs`, env-first Proton and launch builders, host-aware probes (git, getent, lspci, kill, system Proton roots), helper scripts using `run_host` / `flatpak-spawn --host` for steam, pgrep, Proton, gamescope, and setsid where needed, removal of the `/usr/bin/rm` fallback with filesystem retry, IPC `launch_platform_status` plus UI badges for Flatpak + blocked `unshare` + profile `network_isolation`, and four new unit tests for `host_std_*` command construction mirroring the async wrapper tests.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                                      |
| ------------- | ---------------- | ------------------------------------------- |
| Complexity    | Large            | Large (touched core, tauri, UI, scripts)    |
| Confidence    | (plan)           | High after `cargo test` + `tsc` + checks   |
| Files Changed | ~17              | 29 (includes lockfile, new hook file)       |

## Tasks Completed

| #   | Task                                              | Status          | Notes                                                                 |
| --- | ------------------------------------------------- | --------------- | --------------------------------------------------------------------- |
| 1   | Extend platform host-execution primitives         | Complete        | `host_std_*`, probes, tests                                           |
| 2   | Env-bearing launch builders                       | Complete        | `runtime_helpers`, `script_runner`, services                          |
| 3   | Migrate sync/async host-binary call sites         | Complete        | taps, settings, diagnostics, tauri kill                               |
| 4   | Validation + system Proton discovery host-aware   | Complete        | `optimizations`, `steam/proton`                                       |
| 5   | Harden helper scripts                             | Complete        | `steam-launch-helper.sh`, `steam-host-trainer-runner.sh`; trainer launcher unchanged (delegates to runner) |
| 6   | Remove `rm` fallback, host-safe kill              | Complete        | `run_executable`, `update`                                            |
| 7   | Persistent Flatpak unshare / network isolation UI | Complete        | `ProfilesPage` hook order fixed for `tsc`                             |
| 8   | Manual Flatpak matrix                             | Not run in CI   | Requires installed Flatpak bundle per plan                            |

## Validation Results

| Level           | Status | Notes                                                                 |
| --------------- | ------ | --------------------------------------------------------------------- |
| Static Analysis | Pass   | `npm exec tsc -- --noEmit` in `src/crosshook-native`; `cargo check -p crosshook-native` |
| Unit Tests      | Pass   | `cargo test -p crosshook-core` (802 tests in suite; platform tests included) |
| Build           | Pass   | `cargo test -p crosshook-native --no-run` (test targets compile)      |
| Integration     | N/A    | No automated integration harness for Flatpak in this run              |
| Edge Cases      | Partial| Manual Task 8 checklist deferred to host environment                  |

## Files Changed

See `git diff --stat` on branch `feat/flatpak-phase-3-process-execution-hardening`. Notable paths:

- `crosshook-core`: `platform.rs`, `runtime_helpers.rs`, `script_runner.rs`, `steam/proton.rs`, `optimizations.rs`, `taps.rs`, `settings/mod.rs`, `export/diagnostics.rs`, `run_executable/service.rs`, `install/service.rs`, `update/service.rs`, `launch/mod.rs`
- `src-tauri`: `commands/launch.rs`, `profile.rs`, `run_executable.rs`, `update.rs`, `lib.rs`
- Frontend: `useLaunchPlatformStatus.ts`, `LaunchPage.tsx`, `ProfilesPage.tsx`, `ThemedSelect.tsx`, `useLibrarySummaries.ts`, `types/library.ts`, mocks
- Scripts: `steam-launch-helper.sh`, `steam-host-trainer-runner.sh`
- `src/crosshook-native/Cargo.lock`

## Deviations from Plan

- **`types/launch.ts`**: Capability types live in `useLaunchPlatformStatus.ts` next to the hook instead of a separate `types/launch.ts` file.
- **`steam-launch-trainer.sh`**: No code changes; detached runner still invokes the bundled `steam-host-trainer-runner.sh`, which now host-wraps Proton/gamescope.
- **Task 8**: Full issue-209 Flatpak matrix not executed in this environment.

## Issues Encountered

- **`ProfilesPage.tsx`**: `showFlatpakNetworkIsolationBadge` referenced `launchPlatform` and `profileNetworkIsolation` before their declarations; fixed by moving `useLaunchPlatformStatus` and `profileNetworkIsolation` state above the callback (hook order preserved).

## Tests Written

| Test file / module   | Tests | Coverage                                      |
| -------------------- | ----- | --------------------------------------------- |
| `platform.rs` tests  | +4    | `host_std_command_with` / `host_std_command_with_env_inner` flatpak vs native |

## Next Steps

- [ ] Code review via `/code-review`
- [ ] Create PR via `/prp-pr`
- [ ] Run `./scripts/build-flatpak.sh --strict` and Task 8 manual matrix on a Flatpak install
