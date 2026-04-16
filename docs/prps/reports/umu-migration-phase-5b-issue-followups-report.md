# Implementation Report: umu-launcher Migration — Phase 5b (Issue follow-ups #242 / #244 / #245)

## Summary

Phase 5b shipped the Phase-5 Open-Question follow-ups. Flathub status for `org.openwinecomponents.umu.umu-launcher` was re-verified as NOT published (resolves #242); no one-click Flathub action was wired. The shipped readiness/install guidance remained upstream-`umu-launcher` focused, while `probe_flatpak_host_umu_candidates` was broadened to look through standard host-side `.local` and pipx `umu-run` locations across home, `/run/host`, and `/var/home` mirrors instead of introducing a launcher-specific probe. The gamescope watchdog stand-down path (resolves #244) now falls back to exe-name-based host-ps descendant discovery using the existing BFS walker; all three outcomes (`capture_file`, `exe_fallback`, `none`) are observable via a structured `fallback` tracing field. A new Steam-Deck caveats surface (resolves #245) was added: `is_steam_deck()` in `platform.rs`, `SteamDeckCaveats` payload on `ReadinessCheckResult`, a new `<section>` in `WizardReviewSummary`, and a persistent `steam_deck_caveats_dismissed_at` RFC3339 dismissal setting with matching Tauri command + triple-state IPC merge + browser-mode mock + dev toggle.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                                                           |
| ------------- | ---------------- | ---------------------------------------------------------------- |
| Complexity    | Medium           | Medium                                                           |
| Confidence    | 8/10             | High (all 16 tasks landed without plan revisions)                |
| Files Changed | 16               | 17 (extra: `lib/toggles.ts` for dev toggle — caught in Task 2.5) |

## Tasks Completed

| #   | Task                                                                         | Status          | Notes                                                                                                                                                 |
| --- | ---------------------------------------------------------------------------- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1.1 | Add `is_steam_deck()` helper to `platform.rs`                                | [done] Complete | +8 tests                                                                                                                                              |
| 1.2 | Add `steam_deck_caveats_dismissed_at` to `AppSettingsData`                   | [done] Complete | +2 tests (backward-compat + roundtrip)                                                                                                                |
| 1.3 | Add `SteamDeckCaveats` type and extend `ReadinessCheckResult`                | [done] Complete | Single existing constructor site updated with `steam_deck_caveats: None`                                                                              |
| 1.4 | Mirror `SteamDeckCaveats` in `types/onboarding.ts`                           | [done] Complete | Pre-emptively added `steam_deck_caveats: null` to mock to keep typecheck green                                                                        |
| 1.5 | Mirror `steam_deck_caveats_dismissed_at` in `types/settings.ts`              | [done] Complete | Field added to `SettingsSaveRequest`, `toSettingsSaveRequest`, and `DEFAULT_APP_SETTINGS`                                                             |
| 1.6 | Expand `probe_flatpak_host_umu_candidates` host-side `umu-run` coverage      | [done] Complete | Added home, `/run/host`, and `/var/home` standard `.local` / pipx candidate paths for Flatpak host probing                                            |
| 2.1 | Extend `readiness.rs` with Steam-Deck caveats + `apply_*_dismissal`          | [done] Complete | +8 tests covering Deck/native-vs-Flatpak caveat behavior, dismissal handling, and `all_passed` invariants                                             |
| 2.2 | Watchdog stand-down fallback + structured outcome tracing                    | [done] Complete | +3 pure-map unit tests; end-to-end coverage deferred to manual Flatpak smoke per plan                                                                 |
| 2.3 | Settings IPC merge for `steam_deck_caveats_dismissed_at`                     | [done] Complete | +4 tests (preserve, set, clear, serialization)                                                                                                        |
| 2.4 | `dismiss_steam_deck_caveats` command + `apply_*` wiring in `check_readiness` | [done] Complete | Registration deferred to Task 4.3 (dead_code warning resolved there)                                                                                  |
| 2.5 | Browser-mode mocks for Steam-Deck caveats                                    | [done] Complete | Added `showSteamDeckCaveats` dev toggle + handler                                                                                                     |
| 3.1 | Expose `steamDeckCaveats` + `dismissSteamDeckCaveats` in `useOnboarding`     | [done] Complete | Additive return-shape update — no reordering of existing keys                                                                                         |
| 4.1 | Render Steam-Deck caveats `<section>` in `WizardReviewSummary.tsx`           | [done] Complete | Non-scrollable `<section>`, BEM-consistent, two buttons (Open docs + Dismiss)                                                                         |
| 4.2 | Prop-drill `steamDeckCaveats` + handler in `OnboardingWizard.tsx`            | [done] Complete | +4 lines; `() => void dismissSteamDeckCaveats()` wrapper as in the umu precedent                                                                      |
| 4.3 | Register `dismiss_steam_deck_caveats` in `src-tauri/src/lib.rs`              | [done] Complete | +1 line; dead_code warning cleared                                                                                                                    |
| 5.1 | Update PRD Decisions + Open Questions + close GitHub issues                  | [done] Complete | Open Questions #1, #3, #4 marked RESOLVED; Phase 5b row added to Implementation Phases + issue tracking. Issue-close via `gh` deferred to post-merge. |

## Validation Results

| Level           | Status      | Notes                                                                                                                                                                        |
| --------------- | ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | [done] Pass | `./scripts/lint.sh` clean — rustfmt, clippy `-D warnings`, biome, tsc, shellcheck all green                                                                                  |
| Unit Tests      | [done] Pass | **1012 Rust tests** pass, 0 fail. 31 new tests added across this phase (matches the per-file breakdown below)                                                                |
| Build           | [done] Pass | `cargo check --all-targets` clean across `crosshook-core`, `crosshook-native`, `crosshook-cli`                                                                               |
| Integration     | [done] Pass | Between-batch validation (type-check + unit tests) ran cleanly after every batch                                                                                             |
| Edge Cases      | [done] Pass | Unit coverage: 16-row readiness matrix, triple-state IPC merge, dismissal rehydrate, exe-name + truncated-cmdline fallback. Manual Flatpak teardown smoke deferred per plan. |

## Files Changed

| File                                                                       | Action  | Lines (approx.)                |
| -------------------------------------------------------------------------- | ------- | ------------------------------ |
| `src/crosshook-native/crates/crosshook-core/src/platform.rs`               | UPDATED | +135                           |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | UPDATED | +42                            |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs`         | UPDATED | +16                            |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`   | UPDATED | +118                           |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | UPDATED | +75                            |
| `src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs`        | UPDATED | +105                           |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`                  | UPDATED | +70                            |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs`                | UPDATED | +41                            |
| `src/crosshook-native/src-tauri/src/lib.rs`                                | UPDATED | +1                             |
| `src/crosshook-native/src/types/onboarding.ts`                             | UPDATED | +11                            |
| `src/crosshook-native/src/types/settings.ts`                               | UPDATED | +5                             |
| `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts`                | UPDATED | +18                            |
| `src/crosshook-native/src/lib/toggles.ts`                                  | UPDATED | +4                             |
| `src/crosshook-native/src/hooks/useOnboarding.ts`                          | UPDATED | +14                            |
| `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`       | UPDATED | +32                            |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`                 | UPDATED | +4                             |
| `docs/prps/prds/umu-launcher-migration.prd.md`                             | UPDATED | +6 (status + resolution notes) |

**Total: 17 files, ~697 lines net added.**

## Deviations from Plan

- **Task 1.4 pre-emptively patched the mock handler.** The plan said "no new mocks or downstream consumers should break yet" for this task. The TypeScript compiler required `steam_deck_caveats: null` on the mock's `check_readiness` return because the field is non-optional nullable (`SteamDeckCaveats | null`, not `steam_deck_caveats?: ...`). Adding a minimal `null` default is the smallest viable fix and aligns with the intended mock contract; Task 2.5 later filled in the conditional toggle-gated payload.
- **Task 1.5 added an extra handler-file field** (`src/lib/mocks/handlers/onboarding.ts`) for the same reason as 1.4 — the mock required a null default to typecheck. Same rationale.
- **Task 1.6 widened the helper's standard `umu-run` search paths** rather than landing a launcher-specific probe. The shipped code stays scoped to executable discovery in common host-side `.local` and pipx layouts (`$HOME`, `/run/host/...`, `/var/home/...`), which keeps readiness aligned with the actual resolver logic instead of implying support for a third-party launcher path CrossHook does not inspect.
- **Task 2.1 kept install guidance upstream-only.** Earlier planning notes assumed a secondary launcher trust line, but the shipped `build_umu_install_advice` logic remains centered on distro-specific `umu-launcher` commands plus upstream docs. `apply_steam_deck_caveats_dismissal` was still exported through `onboarding/mod.rs` for consumer use as planned.
- **Task 2.2 could not end-to-end unit-test `resolve_watchdog_target`** because it shells out to host `ps` and `/proc/<pid>/...` readers. Pure unit tests cover the same logic via the primitives (`collect_descendant_pids_from_children_map` + `comm_matches_candidates` + `process_name_candidates`). End-to-end coverage of the exe-name fallback depends on manual Flatpak + gamescope teardown smoke, which the plan flagged in advance as the expected validation path.
- **Task 2.4's `dismiss_steam_deck_caveats` emitted a `dead_code` warning** in the interim between Task 2.4 and Task 4.3. This was anticipated; Task 4.3 registered the command in `invoke_handler`, and the warning cleared immediately. Final clippy run is clean.
- **Task 5.1 deferred GitHub issue closure** to post-PR-merge — the plan suggested closing during implementation, but closing open issues before the PR lands risks stranding users who follow activity. Issue comments will reference the merged PR.

## Issues Encountered

- **Parallel dispatch produced two transient type errors** where Tasks 1.4 and 1.5 ran before each other's changes landed. Specifically: Task 1.4 added `SteamDeckCaveats | null` on `ReadinessCheckResult`, and until it did, the mock handler typechecked without the field. Once both landed, between-batch validation was green. No manual intervention needed.

## Tests Written

| Test File                                                                  | Tests | Coverage                                                                                        |
| -------------------------------------------------------------------------- | ----- | ----------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/platform.rs`               | 8 new | `is_steam_deck_from_sources` — env, os-release ID, VARIANT_ID (quoted/single-quoted), negatives |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`           | 2 new | backward-compat-without-field + roundtrip                                                       |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`   | 8 new | caveats present/absent matrix, dismissal clear/noop, and `all_passed` invariance                |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | 3 new | standard host `umu-run` candidate coverage and probe preference for discovered home-local paths |
| `src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs`        | 3 new | exe-name match, no-match, truncated-comm cmdline fallback                                       |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`                  | 4 new | triple-state merge (preserve/set/clear) + serialization containment                             |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs`                | 3 new | signature freeze, store mutation, check_readiness applies caveats dismissal                     |

**Total: 31 new tests added. 0 existing tests removed. 3 existing tests updated for the new `evaluate_checks_inner` bool parameter.**

## Next Steps

- [ ] Code review via `/ycc:code-review` before commit/PR.
- [ ] Create PR via `/ycc:prp-pr`.
- [ ] Manual Flatpak smoke test on a Flatpak host: gamescope + umu + kill gamescope from outside, verify watchdog logs `fallback = "capture_file"` or `"exe_fallback"` and the Wine game PID no longer survives.
- [ ] Post-merge: close GitHub issues #242, #244, #245 with comments referencing the merged PR.
- [ ] (Optional) Manual Steam Deck smoke test on actual hardware (desktop + gaming mode) to confirm caveats render and dismiss survives reload.
