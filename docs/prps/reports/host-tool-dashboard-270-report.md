# Implementation Report: Host Tool Dashboard + Shared Capability Gating

## Summary

Implemented a shared host-tool readiness stack across `crosshook-core`, Tauri IPC, browser mocks, frontend hooks, and the Settings/onboarding UI. The work adds per-tool detail probing, a capability-derivation model, a Settings-hosted host tool dashboard, onboarding handoff into that dashboard, and capability-based panel messaging for Gamescope, MangoHud, Steam launch options, launch optimizations, and prefix-tool dependency hooks.

## Assessment vs Reality

| Metric        | Predicted (Plan)                 | Actual                                                  |
| ------------- | -------------------------------- | ------------------------------------------------------- |
| Complexity    | Medium-High                      | High                                                    |
| Confidence    | Medium                           | Medium                                                  |
| Files Changed | Multi-layer Rust + Tauri + React | 20 modified, 12 new tracked files, 2 local ignored docs |

## Tasks Completed

| #   | Task                         | Status          | Notes                                                                                                                      |
| --- | ---------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------------------- |
| 0   | Discovery / notes            | [done] Complete | Consolidated Batch 1 findings locally to avoid parallel write conflicts on a single notes file                             |
| A   | Core capability model        | [done] Complete | Added capability map, detail probing, and additive readiness fields                                                        |
| B   | IPC + data hook              | [done] Complete | Added snapshot/detail/capability commands plus shared frontend hooks                                                       |
| C   | Dashboard UI                 | [done] Complete | Added dashboard primitives, composition, Settings mount, onboarding handoff, and scroll registration                       |
| D   | Gating wiring                | [done] Complete | Added capability-driven messaging to Gamescope, MangoHud, Launch Optimizations, Steam launch options, and prefix-tool hook |
| E   | Preferences / stale handling | [done] Complete | Added TOML-backed dashboard preference fields and boot-time live refresh over cached readiness                             |
| F   | Docs / acceptance            | [done] Complete | Added report, research cross-link, and local internal docs                                                                 |

## Validation Results

| Level           | Status      | Notes                                                                                                                       |
| --------------- | ----------- | --------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | [done] Pass | `cargo fmt --check`, `cargo check --manifest-path src/crosshook-native/Cargo.toml`, `npm run typecheck`, `npm run lint`     |
| Unit Tests      | [done] Pass | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`                                              |
| Build           | [done] Pass | `npm run build`                                                                                                             |
| Integration     | [done] Pass | `cargo test --manifest-path src/crosshook-native/src-tauri/Cargo.toml --lib command_signatures_match_expected_ipc_contract` |
| Edge Cases      | [done] Pass | Reviewed/fixed cached-dismissal reloads and stale-startup refresh behavior after code review                                |

## Files Changed

| File                                                                            | Action  | Lines      |
| ------------------------------------------------------------------------------- | ------- | ---------- |
| `docs/research/flatpak-bundling/14-recommendations.md`                          | UPDATED | +9 / -0    |
| `src/crosshook-native/assets/default_capability_map.toml`                       | CREATED | new        |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs`       | CREATED | new        |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs`          | CREATED | new        |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs`              | UPDATED | +13 / -0   |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`        | UPDATED | +4 / -0    |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`                | UPDATED | +56 / -0   |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs`                     | UPDATED | +184 / -2  |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`                       | UPDATED | +12 / -0   |
| `src/crosshook-native/src-tauri/src/lib.rs`                                     | UPDATED | +9 / -1    |
| `src/crosshook-native/src/App.tsx`                                              | UPDATED | +6 / -0    |
| `src/crosshook-native/src/components/GamescopeConfigPanel.tsx`                  | UPDATED | +40 / -1   |
| `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`              | UPDATED | +80 / -3   |
| `src/crosshook-native/src/components/MangoHudConfigPanel.tsx`                   | UPDATED | +40 / -1   |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`                      | UPDATED | +44 / -1   |
| `src/crosshook-native/src/components/SettingsPanel.tsx`                         | UPDATED | +10 / -0   |
| `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`               | UPDATED | +17 / -0   |
| `src/crosshook-native/src/components/host-readiness/CapabilitySummaryStrip.tsx` | CREATED | new        |
| `src/crosshook-native/src/components/host-readiness/HostDelegationBanner.tsx`   | CREATED | new        |
| `src/crosshook-native/src/components/host-readiness/HostToolCard.tsx`           | CREATED | new        |
| `src/crosshook-native/src/components/host-readiness/HostToolDashboard.tsx`      | CREATED | new        |
| `src/crosshook-native/src/components/host-readiness/HostToolFilterBar.tsx`      | CREATED | new        |
| `src/crosshook-native/src/hooks/useCapabilityGate.ts`                           | CREATED | new        |
| `src/crosshook-native/src/hooks/useHostReadiness.ts`                            | CREATED | new        |
| `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts`               | UPDATED | +12 / -2   |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                            | UPDATED | +1 / -1    |
| `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts`                     | UPDATED | +460 / -86 |
| `src/crosshook-native/src/styles/host-tool-dashboard.css`                       | CREATED | new        |
| `src/crosshook-native/src/styles/variables.css`                                 | UPDATED | +9 / -0    |
| `src/crosshook-native/src/types/onboarding.ts`                                  | UPDATED | +27 / -0   |
| `src/crosshook-native/src/types/settings.ts`                                    | UPDATED | +8 / -0    |

## Deviations from Plan

- Batch 1 was still executed in parallel for discovery, but the final notes were consolidated locally because all three discovery tasks targeted the same output file.
- `SettingsPanel` briefly drifted into a duplicated local dashboard composition during parallel integration and was normalized back to the single shared `HostToolDashboard` component before final validation.
- `docs/internal/host-tool-dashboard.md` and `docs/internal/host-tool-dashboard-notes.md` were produced as local documentation, but this repo ignores `docs/internal/` in Git, so they are not tracked artifacts.
- The review surfaced a medium-severity follow-up that is outside the strict plan slice: `LaunchPage` still does not consume `prefixToolsCapabilityState` / `prefixToolsRationale` from `useLaunchPrefixDependencyGate()`.

## Issues Encountered

- Git branch creation with slash-separated names failed in this sandbox because `.git/refs/heads/feat/...` could not be created; the working branch was created as `feat/host-tool-dashboard` instead.
- Parallel worker output for Batch 3 overreached by re-implementing capability logic in the Tauri layer; that was refactored back to `crosshook-core` as the sole source of truth before validation.
- Post-review fixes were required for cached snapshot dismissal overlays and boot-time live refresh behavior.

## Tests Written

| Test File                                                                 | Tests                               | Coverage                                                                    |
| ------------------------------------------------------------------------- | ----------------------------------- | --------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/capability.rs` | Added derivation + map loader tests | Capability parse/merge/override and available/degraded/unavailable fixtures |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/details.rs`    | Added parser table test             | Known host-tool version line parsing                                        |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`          | Added 2 tests                       | Backward compatibility + roundtrip for dashboard TOML fields                |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs`               | Added / extended targeted tests     | IPC command signatures and cached snapshot decoding/sanitization            |

## Next Steps

- [ ] Decide whether to finish the remaining medium-severity follow-up by consuming `prefixToolsCapabilityState` / `prefixToolsRationale` in `LaunchPage`.
- [ ] Create a code-review artifact if a formal PR review document is still required.
- [ ] Create the PR from `feat/host-tool-dashboard`.
