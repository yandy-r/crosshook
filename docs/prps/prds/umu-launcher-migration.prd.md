# umu-launcher Migration for Non-Steam Runtime

## Problem Statement

CrossHook's non-Steam Windows-game launch path assembles Proton invocations directly (`"$PROTON" run <exe>` with hand-built env), which means users pay the full cost of Proton-version management: per-game compatibility tweaks, proton-hopping to find a working build, manual `PROTON_*`/`DXVK_*` tuning, and a growing native-vs-Flatpak divergence in the helper shell scripts. CrossHook is a **hybrid** Steam + non-Steam launcher — non-Steam users own real games (Epic, GOG, Humble, itch, sideloaded) and deserve the same "it just works" experience Steam users get. The cost of not solving it: continued issue-tracker burden on `area:launch`, slow onboarding for new users, and competitive pressure from Lutris (0.5.20 defaults to umu + GE-Proton) and Heroic (2.16 defaults to umu) that already took this step.

## Evidence

- **Prior attempt exists**: Issue #140 + PR #148 (`feat(launch): prefer umu-run for proton helper flows`) was merged and reverted hours later as commit `e5f182c`. Revert reason: _"umu-run's container/session management (pressure-vessel) blocks until the entire Wine process tree exits, causing trainers to hang until the game closes."_ The revert commit does **not** touch `PROTON_VERB` — strong evidence the prior attempt used umu's default `waitforexitandrun` for trainers instead of the documented `PROTON_VERB=runinprefix` escape hatch.
- **Residual umu scaffolding** already lives in the code despite the revert: `resolve_umu_run_path()` at `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs:547-558`, `ProtonSetup.umu_run_path` at `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs:79-80`, onboarding health check at `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs:125-151` (now stale), and `GAMEID` plumbing at `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:875-896`.
- **Ecosystem has converged**: Lutris 0.5.20 ships umu as the default Proton runtime; Heroic 2.16 ships umu as default. Both are reference points for hybrid-launcher UX.
- **Assumption — needs validation**: adoption pain among the user base is qualitative ("Proton complexity", "some games work better than others"). No concrete issue-count baseline has been established — the MVP success signal leans on positive user feedback + qualitative `area:launch` trend rather than a hard percentage.

## Proposed Solution

Reattempt umu-launcher adoption — but **phased, testable, and reversible** — migrating only the non-Steam runtime path (`METHOD_PROTON_RUN` arm). Keep Steam-applaunch and Steam-via-Flatpak-helper flows on today's direct Proton path to avoid two runtime containers colliding. Keep `"$PROTON" run` as an ongoing compatibility fallback because some titles still do not work reliably under `umu-run` (for example, _The Witcher 3_). The architectural choice is to **branch inside existing builders** (`build_proton_game_command`, `build_proton_trainer_command`) on a user `UmuPreference` + `resolve_umu_run_path()`, rather than adding a fourth dispatch method — this keeps the method enum, preview layer, CLI, settings migration, and frontend types untouched. On Flatpak, use a **host-shared umu runtime** pattern (mirroring Lutris's `--filesystem=xdg-data/umu:create`) and guide first-run installation from within CrossHook — no bundling, no duplicate 1.5GB SLR downloads per sandboxed app.

## Key Hypothesis

We believe **using `umu-run` for non-Steam game launches and trainer injection** will **reduce Proton-compatibility friction and support-burden for hybrid CrossHook users across native + Flatpak installs**.
We'll know we're right when, in the 2 minor releases after the Phase 4 default-on ship, we see **positive user feedback on frictionless non-Steam launches and a qualitative decline in `area:launch` compatibility issues** reported against non-Steam profiles.

## What We're NOT Building

- **Steam-profile migration** — Steam's Proton runtime stays unchanged. Running umu on Steam profiles risks two pressure-vessel containers colliding.
- **Custom-Proton-fork / tinkerer support** — users who want to hand-assemble env vars keep `UmuPreference::Proton` as an explicit opt-out, but the UX is not tuned for them.
- **Non-x86_64 architectures** — deferred until user demand materializes. umu's container is x86_64 / x86_64-i386.
- **`winetricks` / `protontricks` migration** — Wine-native; does not need umu wrapping. `src/crosshook-native/crates/crosshook-core/src/prefix_deps/runner.rs:86-208` stays as-is.
- **Auto-resolution of `GAMEID` via the umu-database HTTP API** — v1 uses Steam-app-id-when-available + `umu-0` fallback (current behavior). Future work, tracked as a separate GitHub issue (see Open Questions).
- **Bundling umu inside CrossHook's Flatpak** — ~100MB + GPL-3 distribution + SLR-bootstrap complexity. Host-shared runtime is the chosen strategy; bundling is deferred out of this PRD.
- **Removing the direct `proton_run` path** — unsupported titles still exist under `umu-run` (for example, _The Witcher 3_), so direct Proton remains a supported compatibility path rather than a temporary migration shim.

## Success Metrics

| Metric                                           | Target                                                                                          | How Measured                                                                                    |
| ------------------------------------------------ | ----------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| Primary: compatibility-issue trend (qualitative) | Decline in open `area:launch` + `type:bug` issues against non-Steam profiles post-Phase 4       | GitHub issue label analytics across 2 minor releases after Phase 4 ship                         |
| Primary: user sentiment                          | Positive user-feedback themes around non-Steam launch ease                                      | GitHub Discussions, release comments, Reddit/Matrix mentions, in-app feedback if/when available |
| Secondary: umu availability coverage             | ≥ 80% of onboarding readiness checks find a usable `umu-run` on native installs                 | Manually sampled from issue reports                                                             |
| Secondary: trainer-injection reliability         | Zero `trainer-hangs-until-game-exits` bug reports attributable to `PROTON_VERB` regression      | GitHub issues tagged `area:injection`                                                           |
| Compatibility fallback health                    | Unsupported titles continue to launch via direct Proton while supported titles benefit from umu | Release retrospectives; compatibility issue triage for known `umu-run` exceptions               |

## Open Questions

- [x] **Is `org.openwinecomponents.umu.umu-launcher` published on Flathub today?** Last confirmed state (upstream issue #335, Jan 2025) was "no Flathub listing." Upstream keeps an in-tree manifest. Must be re-checked at Phase 5 implementation start; determines whether CrossHook's "install UMU" dialog can offer a one-click Flathub action.
      **RESOLVED (2026-04-15, Phase 5b / issue #242)**: Re-verified — still NOT published on Flathub (`https://flathub.org/apps/org.openwinecomponents.umu.umu-launcher` returns 404). Onboarding keeps distro-aware install commands + upstream GitHub releases link. No one-click Flathub action wired; follow-up issue will re-open if upstream publishes.
- [x] **Exact `PROTONPATH` format** — umu accepts both a directory path and a tag name (`GE-Proton`, `GE-Proton9-20`). CrossHook stores the Proton executable path. **Decision (2026-04-14, issue #243)**: use `dirname(request.runtime.proton_path)`. Tag-name form rejected — it risks an implicit GitHub-releases fetch, breaks custom/hand-placed Proton builds (Valve Proton, Proton-GE-Custom, handmade forks), and would duplicate storage when umu's tag resolver downloads into its own cache. `dirname` preserves the user's explicit Proton choice, works offline, and composes with the Phase 2 pressure-vessel allowlist that already makes the Proton dir sandbox-visible. See `docs/prps/plans/umu-migration-phase-3-umu-opt-in.plan.md` §Decision #243.
- [x] **Gamescope → pressure-vessel SIGTERM propagation** — empirically works (compositor session death kills children via lost Wayland socket); formally undocumented. If a Phase 3 bug turns up here, CrossHook's watchdog may need to target the game Wine PID inside the container instead of gamescope.
      **RESOLVED (2026-04-15, Phase 5b / issue #244)**: Added exe-name-based host-PID fallback in `resolve_watchdog_target` using the existing host-ps BFS (`collect_host_descendant_pids` + `host_process_matches_candidates`, including `TASK_COMM_LEN=15` cmdline fallback). Tracing now carries `fallback = "capture_file" | "exe_fallback" | "none"` together with `game_exe`, `observed_gamescope_pid` (the gamescope/root PID the walker starts from), and `discovered_pid` when a shutdown target was found (`capture_file`, `exe_fallback`). On `exe_fallback` and `none`, `observed_descendants` records the descendant count from that walk (omitted on the `capture_file` path, which resolves before the exe-name walk). Standing-down is now last-ditch, not first-response.
- [x] **Steam Deck gaming-mode edge cases** — umu + gamescope + Flatpak + SteamOS 3.8+ has open upstream issues (black screen without Shader Pre-Caching, Steam overlay z-order, HDR regression on 3.7.13). CrossHook should document these in Phase 5 onboarding guidance, not solve them upstream-side.
      **RESOLVED (2026-04-15, Phase 5b / issue #245)**: Added `crosshook_core::platform::is_steam_deck()` (env `SteamDeck`/`SteamOS` + os-release `ID=steamos` / `VARIANT_ID=steamdeck`, with `/run/host/etc/os-release` fallback for Flatpak). New `SteamDeckCaveats` payload flows through `ReadinessCheckResult` into a dedicated `<section>` in `WizardReviewSummary` with three documented caveats, "Open docs" + "Dismiss" buttons, and a persisted `steam_deck_caveats_dismissed_at` RFC3339 setting. Dismissal is one-shot (matches `install_nag_dismissed_at`); no automated workarounds landed.
- [ ] **Telemetry scope** — CrossHook today has no anonymous telemetry for launch outcomes. Establishing a baseline for the "issue decline" success metric may require building one, or the metric stays purely qualitative. Flagged but not required for PRD.
- [x] **GAMEID umu-database HTTP lookups (future)** — v1 ships with Steam-app-id-when-available + `umu-0` fallback. If protonfix miss rate on non-Steam titles becomes user pain, a separate GitHub issue tracks a Phase 7+ design (HTTP client, cache persistence as SQLite metadata with TTL, offline degradation). **To be filed as a new `type:feature` GitHub issue before or immediately after this PRD lands.**  
       **RESOLVED (2026-04-14)**: #247 covers the cache layer (full CSV fetch + ETag revalidation). Per-id HTTP endpoints remain deferred; #251 closed as duplicate of #247. Phase 3b ships the full-CSV cache.

---

## Users & Context

**Primary User** — mix of two overlapping profiles:

- **Native power user** on Arch, Fedora, EndeavourOS, CachyOS, openSUSE Tumbleweed who already manages Proton-GE themselves. umu is usually one `pacman -S` or equivalent away; onboarding friction is near-zero. They want CrossHook to use umu because it removes busywork they're currently doing by hand.
- **Flatpak-first user** on Steam Deck desktop, Bazzite, Aurora, Bluefin, uBlue. Their host package manager is immutable-ish; they install apps through Discover / Flathub. For them, umu adoption has to work either via a host-installed umu (best case) or a clearly-guided install path CrossHook walks them through.

**Who**: hybrid launcher users with a mix of Steam and non-Steam Windows-game ownership — Epic, GOG, Humble, itch, Battle.net, sideloaded ISO, etc.
**Current behavior**: wrestle with Proton versions, hand-apply workarounds, ask for help in CrossHook issues or Discord/Matrix, occasionally give up and use Lutris as a competitor.
**Trigger**: "I just added a non-Steam game to CrossHook and want to launch it — ideally with a trainer attached."
**Success state**: game boots on first try with no user-visible Proton-version thinking; trainer attaches cleanly alongside; user moves on to playing.

**Job to Be Done**
When I add a non-Steam Windows game to CrossHook, I want it to pick the right Proton build and apply known fixes automatically, so I can launch the game flawlessly — with or without a trainer — without troubleshooting runtime issues first.

**Non-Users**
Steam-profile users (covered by Steam's own runtime), custom-Proton-fork users who want hand-assembled env, non-x86_64 users (no umu support), and tinkerers who explicitly prefer managing Proton themselves (they keep `UmuPreference::Proton` as an opt-out but aren't the design target).

---

## Solution Detail

### Core Capabilities (MoSCoW)

| Priority | Capability                                                                                                                                      | Rationale                                                                                                           |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Must     | `umu-run` launches non-Steam games on native installs with `PROTON_VERB=waitforexitandrun`                                                      | The primary function — games must launch                                                                            |
| Must     | `umu-run` launches non-Steam trainers with `PROTON_VERB=runinprefix` (avoiding the PR #148 regression)                                          | Trainer injection is the core CrossHook value-add; the prior failure mode must not recur                            |
| Must     | Flatpak CrossHook uses host-shared umu runtime via `--filesystem=xdg-data/umu:create` + guided first-run install when umu is missing            | Large Linux-Flatpak audience; shared runtime with Lutris/Heroic avoids 1.5GB duplicate downloads                    |
| Must     | Pressure-vessel filesystem allowlist (`STEAM_COMPAT_LIBRARY_PATHS`, `PRESSURE_VESSEL_FILESYSTEMS_RW`) covers game dir, trainer dir, working dir | Trainers outside `$HOME` must remain visible inside the sandbox                                                     |
| Must     | `UmuPreference { Auto, Umu, Proton }` — default `Auto` prefers umu when present, degrades to Proton otherwise                                   | Backward compat is HIGH — no existing user's setup can break                                                        |
| Must     | Fallback path: when `umu-run` is absent or `UmuPreference::Proton`, builders cleanly emit today's direct Proton command                         | Risk mitigation during rollout                                                                                      |
| Should   | Exported launcher scripts emit `umu-run` with `command -v umu-run` runtime probe and `$PROTON` fallback header                                  | Users who share exported scripts should get umu benefits without breaking others' hosts                             |
| Should   | Preview UI renders `umu-run` vs `"$PROTON" run` so power users see which path is active                                                         | Transparency; debugging; onboarding feedback                                                                        |
| Should   | Onboarding readiness check upgraded from stale Info to actionable (install-help dialog when Flatpak + no host umu)                              | Flatpak users need first-run guidance                                                                               |
| Should   | Per-profile `runtime.umu_game_id: Option<String>` override in TOML for manual protonfix mapping                                                 | Escape hatch when umu-database lookup doesn't apply (note: v1 has no HTTP lookup; this field is user-editable only) |
| Could    | Telemetry baseline for launch outcomes                                                                                                          | Would upgrade the success metric from qualitative to quantitative                                                   |
| Could    | HTTP umu-database `GAMEID` resolver with SQLite-cached lookups                                                                                  | Improves protonfix hit rate for non-Steam titles — tracked as a separate future GitHub issue, not v1                |
| Won't    | Steam-profile migration to umu                                                                                                                  | Two pressure-vessel containers; explicitly out of scope                                                             |
| Won't    | Bundling umu inside CrossHook Flatpak (replicating what Faugus does)                                                                            | Size, license, bootstrap complexity — host-shared runtime is strictly better                                        |
| Won't    | Non-x86_64 support                                                                                                                              | No umu arch support today                                                                                           |
| Won't    | `winetricks` / `protontricks` wrapping via umu                                                                                                  | Wine-native; does not need it                                                                                       |

### MVP Scope

**MVP = Phases 1 through 4** (see Implementation Phases table below).

The public-visible "umu is here" moment is **Phase 4** (default-on `UmuPreference::Auto` + exported-script parity). Phases 1–3 are a mix of invisible hygiene (1), inert-under-Proton plumbing (2), and opt-in user-testable behavior (3). Phase 5 (Flatpak runtime + install guidance) is a **must-have v1** capability but ships as its own phase so it can iterate independently on Flatpak-specific surface area.

### User Flow

1. User opens CrossHook.
2. Onboarding readiness check detects `umu-run` on PATH (native) or inside sandbox (Flatpak with host `~/.local/share/umu` visible). If missing on Flatpak, dialog offers distro-aware install commands or Flathub links.
3. User adds a non-Steam Windows game profile (existing flow; nothing changes here).
4. User clicks "Launch". Preview shows `umu-run <game.exe>` with resolved `PROTONPATH` / `GAMEID` / `PROTON_VERB`.
5. Game boots. Trainer (if configured) launches alongside via `umu-run <trainer.exe>` with `PROTON_VERB=runinprefix` in the same prefix — both visible simultaneously in `ps`.
6. If umu is missing or `UmuPreference::Proton` is set, Step 4 preview shows `"$PROTON" run <game.exe>` and the existing direct-Proton path runs unchanged.

---

## Technical Approach

**Feasibility**: **HIGH** — the existing architecture maps cleanly onto a umu swap, the prior attempt's failure mode is understood and fixable with a one-line env change per builder, and the hardest problem (Flatpak distribution) has a known-good pattern (Lutris's shared-runtime manifest).

**Architecture Notes**

- **Branch inside existing builders**, keep `METHOD_PROTON_RUN` — no new dispatch method. `build_proton_game_command` at `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs:401-484` and `build_proton_trainer_command` at `script_runner.rs:486-566` both gain a `use_umu: bool` decision derived from `UmuPreference` + `resolve_umu_run_path().is_some()`. Steam-context callers (`build_flatpak_steam_trainer_command` at `script_runner.rs:388-399`) explicitly opt out.
- **`PROTON_VERB` is the critical new env var** — set at builder level (not via the optimization directive pipeline). Game builder → `waitforexitandrun`. Trainer builders → `runinprefix`. Add `PROTON_VERB` to `WINE_ENV_VARS_TO_CLEAR` at `src/crosshook-native/crates/crosshook-core/src/launch/env.rs:8-40` to prevent host leakage.
- **`PROTONPATH` derivation** — umu wants the Proton directory or a tag (`GE-Proton9-20`). CrossHook stores the executable path. Use `dirname(request.runtime.proton_path)` as the first-cut implementation; validate against tag shortcuts during Phase 3.
- **Pressure-vessel filesystem allowlist** — new helper `collect_pressure_vessel_paths(request) -> Vec<String>` returns deduplicated `{dirname(game_path), dirname(trainer_host_path) when SourceDirectory mode, working_directory}`, colon-joined into `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW`. Inert under direct Proton (Proton doesn't consume these); safe to ship in Phase 2 before any umu path activates.
- **Gamescope PID capture** — no change. `FLATPAK_GAMESCOPE_PID_CAPTURE_SCRIPT` at `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs:19-20` still captures the shell `$$` that `exec`s gamescope; umu inserts beneath gamescope, not around it. SIGTERM to gamescope PID collapses the session and the child tree dies with lost Wayland. Formally undocumented — tracked as an open question.
- **Flatpak strategy** — add `--filesystem=xdg-data/umu:create` to `packaging/flatpak/dev.crosshook.CrossHook.yml`, matching Lutris's pattern (`net.lutris.Lutris` manifest). Invoke via existing `host_command_with_env` / `flatpak-spawn --host umu-run` at `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs:56-77`. When host umu is missing, onboarding upgrades from Info to actionable install guidance.
- **Fallback boundary** — decided at **command-build time** (inside the two builders), not exec time. Preview uses the same `resolve_umu_run_path()` so users see `umu-run` vs `"$PROTON" run` in the preview before they click Launch. No fail-then-retry UX.

**Technical Risks**

| Risk                                                                                                                                  | Likelihood                | Mitigation                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PR #148 regression recurs (trainers hang until game exit)                                                                             | H                         | **Phase 1 lands `PROTON_VERB=runinprefix` for trainers before any umu code path goes live.** Matrix tests assert env per builder; E2E verifies concurrent PIDs                                |
| Trainer paths outside `$HOME` invisible inside pressure-vessel sandbox                                                                | H                         | Phase 2 ships pressure-vessel allowlist plumbing before Phase 3 activates umu. Prefer `stage_trainer_into_prefix` (already exists) — path stays inside prefix                                 |
| `org.openwinecomponents.umu.umu-launcher` not on Flathub; Flatpak users cannot easily self-install                                    | M                         | Phase 5 onboarding dialog offers distro-aware install commands + links to Faugus Launcher (umu-carrying Flatpak) + upstream releases. One-click Flathub path added if/when upstream publishes |
| `PROTONPATH` directory vs tag format mismatch breaks user GE-Proton builds                                                            | M                         | Phase 3 validates `dirname(proton_path)` against umu's PROTONPATH search; if tag is required, extend `resolve_launch_proton_path` to emit both forms                                          |
| Gamescope SIGTERM does not cleanly tear down pressure-vessel child tree                                                               | L                         | Empirically works today; if Phase 3 surfaces a bug, watchdog can be extended to walk `ps --ppid` into the container. Tracked as open question                                                 |
| Steam Deck gaming-mode edge cases (Shader Pre-Caching, overlay z-order, HDR on SteamOS 3.7.13) confuse users                          | M                         | Phase 5 onboarding documents known workarounds; does not attempt to solve upstream issues                                                                                                     |
| Test churn — ~20 existing `crosshook-core` tests assert `"$PROTON" run` in command args and will need split assertions for both paths | L                         | Mechanical volume; covered in phase test plans. Keep original assertions for Proton fallback branch; add sibling assertions for umu branch                                                    |
| GPL-3 umu + CrossHook license interaction if bundling ever revisited                                                                  | L (out of scope this PRD) | Deferred with Phase 5b; subprocess invocation is aggregation, not derived work — documented for future re-evaluation                                                                          |

---

## Implementation Phases

<!--
  STATUS: pending | in-progress | complete | will-not-implement
  PARALLEL: phases that can run concurrently (e.g., "with 3" or "-")
  DEPENDS: phases that must complete first (e.g., "1, 2" or "-")
  PRP: link to generated plan file once created
-->

| #   | Phase                                              | Description                                                                                                                                                                                                                   | Status             | Parallel | Depends | PRP Plan                                                                                                                                                               |
| --- | -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------ | -------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | PROTON_VERB hygiene                                | Set `PROTON_VERB=waitforexitandrun` for game builder, `runinprefix` for trainer builders; add to `WINE_ENV_VARS_TO_CLEAR`                                                                                                     | complete           | -        | -       | [plan](../plans/completed/umu-migration-phase-1-proton-verb-hygiene.plan.md)                                                                                           |
| 2   | Sandbox allowlist plumbing                         | Collect `{game_dir, trainer_dir, working_dir}`; set `STEAM_COMPAT_LIBRARY_PATHS` + `PRESSURE_VESSEL_FILESYSTEMS_RW` (inert under Proton)                                                                                      | complete           | with 1   | -       | [plan](../plans/completed/umu-migration-phase-2-sandbox-allowlist.plan.md), [report](../reports/umu-migration-phase-2-sandbox-allowlist-report.md)                     |
| 3   | umu opt-in (non-Steam only)                        | Add `UmuPreference` setting; branch builders on `Umu` + `resolve_umu_run_path()`; derive `PROTONPATH`; Steam paths explicitly opt out                                                                                         | complete           | -        | 1, 2    | [plan](../plans/umu-migration-phase-3-umu-opt-in.plan.md), [report](../reports/umu-migration-phase-3-umu-opt-in-report.md)                                             |
| 3b  | umu-database coverage warning + HTTP cache         | CSV coverage (`CsvCoverage`) in `UmuDecisionPreview`; amber chip + badge when `will_use_umu && csv_coverage === missing`; background HTTP refresh via `external_cache_entries`; Settings refresh button (#263, #247)          | complete           | -        | 3       | [plan](../plans/umu-migration-phase-3b-umu-opt-in.plan.md), [report](../reports/umu-migration-phase-3b-umu-opt-in-report.md)                                           |
| 4   | Auto-default + exported-script parity              | `UmuPreference::Auto` prefers umu when present; `build_exec_line` emits `command -v umu-run` probe with `$PROTON` fallback. _Prerequisite: #263 + #247 landed AND 2-week observation clean._                                  | complete           | -        | 3, 3b   | [plan](../plans/completed/umu-migration-phase-4-auto-default.plan.md), [report](../reports/umu-migration-phase-4-auto-default-report.md)                               |
| 5   | Flatpak host-shared umu runtime + install guidance | Add `--filesystem=xdg-data/umu:create`; upgrade onboarding readiness from Info to actionable install-help dialog                                                                                                              | complete           | with 4   | 3       | [plan](../plans/completed/umu-migration-phase-5-flatpak-host-shared-runtime.plan.md), [report](../reports/umu-migration-phase-5-flatpak-host-shared-runtime-report.md) |
| 5b  | Issue follow-ups (#242 / #244 / #245)              | Phase 5 shipped; Phase 5b resolves the open-question issues: Flathub status (not published → keep distro commands + upstream link), gamescope→PV teardown fallback by exe-name, Steam-Deck gaming-mode caveats in onboarding. | complete           | -        | 5       | [plan](../plans/completed/umu-migration-phase-5b-issue-followups.plan.md), [report](../reports/umu-migration-phase-5b-issue-followups-report.md)                       |
| 6   | Remove `proton_run` direct path                    | Retired. Some games remain incompatible with `umu-run` (for example, _The Witcher 3_), so direct Proton stays as a supported fallback.                                                                                        | will-not-implement | -        | -       | -                                                                                                                                                                      |

### Phase Details

**Phase 1: PROTON_VERB hygiene**

- **Goal**: Establish correct `PROTON_VERB` semantics **before** any umu code path activates, so the PR #148 regression mode is architecturally impossible to recreate.
- **Scope**: Builder-level env in `build_proton_game_command` (`waitforexitandrun`) and `build_proton_trainer_command` + `build_flatpak_steam_trainer_command` (`runinprefix`). Add `PROTON_VERB` to `WINE_ENV_VARS_TO_CLEAR` at `env.rs:8-40`. Mirror unset in `runtime-helpers/steam-host-trainer-runner.sh:450-460`. Zero observable behavior change under direct Proton (Proton's default verb is `waitforexitandrun`; `runinprefix` in secondary invocations is still well-formed direct-Proton).
- **Success signal**: All existing tests green. New tests assert `command_env_value(&command, "PROTON_VERB")` per builder. Preview renders `PROTON_VERB` for both game and trainer.
- **Tests added**: Sibling tests at `script_runner.rs:1048` and `script_runner.rs:1132` covering env for each builder.

**Phase 2: Sandbox allowlist plumbing**

- **Goal**: Make trainer paths outside `$HOME` reachable when pressure-vessel eventually wraps execution, without changing today's runtime.
- **Scope**: New pure helper `collect_pressure_vessel_paths(request) -> Vec<String>` in `runtime_helpers.rs`. Set `STEAM_COMPAT_LIBRARY_PATHS` + `PRESSURE_VESSEL_FILESYSTEMS_RW` colon-joined. These vars are inert under direct Proton — zero behavior change.
- **Success signal**: Env contains expected colon-joined path list. Existing fixture paths (`/games/My Game/game.exe`, `/trainers/trainer.exe`) surface in allowlist output. Proton path still works identically.
- **Tests added**: Unit tests on the collector (empty request, full request, SourceDirectory vs CopyToPrefix modes); env assertions on both builders.
- **Artifacts**: [plan](../plans/completed/umu-migration-phase-2-sandbox-allowlist.plan.md), [report](../reports/umu-migration-phase-2-sandbox-allowlist-report.md)

**Phase 3: umu opt-in (non-Steam only)**

- **Goal**: First real umu code path, behind explicit `UmuPreference::Umu`, exercisable by adventurous users and integration tests.
- **Scope**:
  - New enum `UmuPreference { Auto, Umu, Proton }` in `settings/mod.rs`, default `Auto` (Phase 3 `Auto` still resolves to Proton — only explicit `Umu` activates the new path).
  - Branch in `build_proton_game_command` and `build_proton_trainer_command` on `use_umu = (preference == Umu || (preference == Auto && Phase 4)) && resolve_umu_run_path().is_some()`.
  - When `use_umu`: swap `"$PROTON" run <target>` for `umu-run <target>`; set `PROTONPATH = dirname(proton_path)` or tag name (validate).
  - Steam-applaunch path (`build_helper_command`) explicitly opts out (no change).
  - `build_flatpak_steam_trainer_command` re-enters `build_proton_trainer_command` — add a flag or gate on `request.method` origin so Steam-context trainers never take the umu branch.
  - Per-profile `runtime.umu_game_id: Option<String>` field in `profile/models.rs`, merged through `local_override` at `profile/models.rs:509-529`.
- **Success signal**: Matrix `{game, trainer} × {umu present+enabled, umu present+auto, umu absent} × {gamescope off/on, flatpak/native}`. Concurrent PID test: launch stub game + stub trainer under `UmuPreference::Umu`; both PIDs alive simultaneously — this is the PR #148 non-regression smoke test.
- **Tests added**: Command-shape split (old assertions → Proton branch; new assertions → umu branch) across ~20 tests in `script_runner.rs`, `runtime_helpers.rs`, `preview.rs`. New E2E: trainer-with-game concurrency test. New fixture profiles for umu path.

**Phase 4: Auto-default + exported-script parity**

- **Goal**: "umu is here" moment — default `UmuPreference::Auto` prefers umu when present. Exported launcher scripts get parity so power users sharing scripts get the same benefit.
- **Scope**:
  - Change `Auto` resolution: if `resolve_umu_run_path().is_some()`, use umu; else Proton.
  - Update `build_exec_line` at `export/launcher.rs:521-551` to emit a runtime `command -v umu-run >/dev/null && exec umu-run "$TRAINER_HOST_PATH" || exec "$PROTON" run "$TRAINER_HOST_PATH"` shape.
  - Update 7 export/launcher.rs test assertions at lines 888, 1010, 1148, 1169, 1185, 1270, 1306 to cover both branches.
  - Release-note + onboarding messaging: "CrossHook now uses umu-launcher when available."
- **Success signal**: Newly exported scripts work on hosts with or without umu. Preview shows `umu-run` when `resolve_umu_run_path()` succeeds. No open regressions tagged `area:launch` against umu path after 1 minor release.
- **Tests added**: Snapshot tests of emitted script content for both branches; integration test with `umu-run` stubbed on/off PATH.

**Phase 5: Flatpak host-shared umu runtime + install guidance**

- **Goal**: Flatpak CrossHook uses host-installed umu without bundling or duplicate runtime downloads; first-run UX guides missing-umu users to install.
- **Scope**:
  - Manifest: add `--filesystem=xdg-data/umu:create` to `packaging/flatpak/dev.crosshook.CrossHook.yml`.
  - Onboarding readiness (`onboarding/readiness.rs:125-151`): upgrade from Info to actionable. When Flatpak + no `umu-run` detected, show Install UMU dialog with:
    - Distro-aware copy-paste install commands (Arch multilib, Fedora COPR if available, Debian community, Nix, AUR).
    - "Install org.openwinecomponents.umu.umu-launcher from Flathub" one-click action **if** upstream has published (re-check at implementation time — see Open Questions).
    - Fallback: link to upstream releases and to Faugus Launcher (umu-carrying Flathub app) as a trusted install vehicle.
  - Settings: `AppSettings.install_nag_dismissed_at: Option<DateTime>` — TOML — so users who intentionally stay on Proton aren't nagged.
- **Success signal**: Flatpak user on any distro reaches "umu-run ready" in ≤ 3 clicks. User who declines stays on Proton with zero broken launches.
- **Tests added**: Onboarding dialog state-machine tests. Manual Flatpak validation: with host umu present / with host umu absent / with Flathub umu installed / after dismissal.

**Phase 6: Remove `proton_run` direct path**

- **Status**: **Will Not Implement**.
- **Rationale**: The migration no longer targets a full hard cutover. Some games are still not supported correctly via `umu-run`, and those profiles need to keep the direct `"$PROTON" run` path available. _The Witcher 3_ is the current concrete example that blocked removal planning.
- **Scope**:
  - Keep the Proton fallback branch in `build_proton_game_command` / `build_proton_trainer_command`.
  - Keep `UmuPreference::Proton` as an explicit compatibility escape hatch.
  - Continue testing both command shapes (`umu-run` and direct Proton) for non-Steam profiles.
  - Keep previously-created Phase 6 tracking issues closed unless new evidence shows full removal is viable again.
- **Success signal**: Unsupported titles remain launchable through direct Proton without blocking umu rollout for titles that do work under `umu-run`.

### Parallelism Notes

- **Phases 1 and 2 can run in parallel** — independent env-plumbing changes in different parts of the request pipeline; merge conflicts limited to shared test files.
- **Phases 4 and 5 can run in parallel** — Phase 4 is a settings default flip + export-script change; Phase 5 is Flatpak manifest + onboarding UI. Both depend on Phase 3 landing but don't depend on each other.
- **Phase 6 is retired** — the PRD now treats direct Proton as a long-lived compatibility path, not a time-gated removal target.

---

## Storage Boundary

Per CLAUDE.md persistence-boundary rule, this feature introduces the following data — classified by storage:

| Datum                                                                  | Classification                     | Rationale                                                                                                                                    |
| ---------------------------------------------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `AppSettings.umu_preference: UmuPreference`                            | **TOML settings** (global)         | User preference, persisted across launches. Default `Auto`. Lives in `AppSettingsData`.                                                      |
| `AppSettings.install_nag_dismissed_at: Option<DateTime>`               | **TOML settings** (global)         | User dismissed install-help dialog; don't re-prompt. Phase 5 only.                                                                           |
| `RuntimeConfig.umu_game_id: Option<String>`                            | **TOML settings** (per profile)    | User-editable override for protonfix lookup when the default (`steam_app_id` or `"umu-0"`) is wrong                                          |
| `LocalOverride.runtime.umu_game_id` (future)                           | **TOML settings** (local override) | Machine-specific umu_game_id override, layered per existing local_override merge at `profile/models.rs:509-529`                              |
| `ProtonSetup.umu_run_path: Option<String>`                             | **Runtime-only** (derived)         | Resolved each preview/launch via `resolve_umu_run_path()`. Already present in code at `preview.rs:79-80`.                                    |
| Pressure-vessel RW path allowlist                                      | **Runtime-only** (derived)         | Computed each launch from `{game_path, trainer_host_path, working_directory}`. Never persisted.                                              |
| (Deferred) `GAMEID` cache from umu-database lookups                    | **SQLite metadata** (future)       | TTL'd lookup cache; only materializes if the HTTP resolver feature is built. Tracked as a future issue.                                      |
| `umu-database` CSV body at `~/.local/share/crosshook/umu-database.csv` | **Operational/cache metadata**     | Persisted cache file on disk, refreshed from upstream; rebuilt from upstream if deleted and tracked via SQLite metadata.                     |
| `external_cache_entries` row (`cache_key="umu-database:csv"`)          | **SQLite metadata**                | ETag + Last-Modified + body_sha256 + cached body metadata for conditional revalidation; ≤1 KB payload, well under `MAX_CACHE_PAYLOAD_BYTES`. |

---

## Persistence & Usability

- **Migration / backward compatibility**:
  - New `umu_preference` field defaults to `Auto` when missing from existing TOML — zero user action required to upgrade. Existing non-Steam profiles keep working under `Auto` + missing umu (which resolves to Proton-fallback).
  - New per-profile `runtime.umu_game_id` is `Option`; absence means "use Steam app_id when present, else `umu-0`" (current behavior).
  - No removal migration is planned for the direct Proton path. `UmuPreference::Proton` remains a supported opt-out because `umu-run` does not cover every title yet.
- **Offline behavior**:
  - v1 has no umu-database HTTP calls — fully offline for `GAMEID` resolution (uses local Steam app_id or fallback).
  - umu itself fetches SteamLinuxRuntime_sniper on first run; this is not a CrossHook-persisted datum. Onboarding readiness flags network requirement on first umu launch.
- **Degraded / failure fallback**:
  - `umu-run` absent → `Auto` resolves to Proton; launch proceeds with today's behavior.
  - `UmuPreference::Proton` forces Proton regardless of umu presence.
  - Fallback decision is made at command-build time and reflected in preview — user sees before they click.
  - umu-run present but fails at exec time → not silently retried (per CLAUDE.md "fail early" convention); user sees Proton output only if `UmuPreference::Proton` is explicitly set.
- **User visibility / editability**:
  - `umu_preference` is editable via Settings UI.
  - Per-profile `umu_game_id` is editable via profile's Runtime section.
  - `install_nag_dismissed_at` is read-only in UI (set when user clicks dismiss); users can reset via Settings reset flow.
  - Derived runtime state (`ProtonSetup.umu_run_path`, pressure-vessel paths) is visible in the Launch Preview view but not editable.

---

## Decisions Log

| Decision                                | Choice                                                                                                                               | Alternatives                                                          | Rationale                                                                                                               |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Migration scope                         | Non-Steam profiles only                                                                                                              | Migrate Steam profiles too                                            | Two pressure-vessel containers; Steam already has a working runtime path. Conflict risk is higher than reward.          |
| Dispatch method evolution               | Keep `METHOD_PROTON_RUN`; branch inside builders                                                                                     | Add fourth `METHOD_UMU_RUN` dispatch method                           | ≥ 8 file touches avoided (preview, CLI, frontend types, settings migration, validation). Cleaner rollout.               |
| Fallback strategy                       | Keep `"$PROTON" run` as a supported compatibility fallback                                                                           | Hard-cutover in one release                                           | Backward compat is HIGH, and some games still require direct Proton today (for example, _The Witcher 3_).               |
| Flatpak distribution                    | Host-shared umu runtime via `--filesystem=xdg-data/umu:create`                                                                       | Bundle umu inside CrossHook Flatpak                                   | ~100MB + GPL-3 + 1.5GB SLR bootstrap per sandboxed app. Shared runtime is strictly better UX and disk cost.             |
| `PROTON_VERB` placement                 | Builder-level env, outside optimization directive pipeline                                                                           | Add `PROTON_VERB` to `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS` allowlist | Verb semantics are inherent to game-vs-trainer distinction, not user-tunable. Belongs with `GAMEID`, not optimizations. |
| Fallback boundary                       | Command-build time (not exec time retry)                                                                                             | Retry on exec failure                                                 | Matches CLAUDE.md "fail early" convention. Preview shows the real command; no surprise path-switches.                   |
| v1 `GAMEID` resolver                    | Steam-app-id-when-available + `umu-0` fallback                                                                                       | HTTP umu-database lookup + SQLite cache                               | Scope-bounded v1; protonfix miss rate may not be user-pain enough to justify now. Tracked as separate future issue.     |
| Bundled umu Flatpak (Phase 5b original) | Deferred out of this PRD                                                                                                             | Ship in v1                                                            | Size + license + SLR bootstrap complexity too high vs. value over host-shared pattern.                                  |
| CSV source precedence                   | HTTP cache → `/usr/share/umu-protonfixes/` → `/usr/share/umu/` → `/opt/umu-launcher/umu-protonfixes/` → `$XDG_DATA_DIRS` → `Unknown` | dirname-only or HTTP-only alternatives                                | Flatpak-safe + offline-first; no host umu-launcher dependency.                                                          |

---

## Research Summary

**Market Context**

- umu-launcher v1.4.0 (March 2026), stable, Open-Wine-Components org, backed by GloriousEggroll.
- Lutris 0.5.20 and Heroic 2.16 both default to umu. Faugus Launcher is umu-centric on Flathub.
- No confirmed Flathub listing for `org.openwinecomponents.umu.umu-launcher` as of upstream issue #335 (Jan 2025). Upstream maintains in-tree manifest. UNVERIFIED for current date.
- umu packaging: Arch multilib, Nixpkgs, AUR, community Debian. No official Fedora.
- Lutris Flathub manifest uses `--filesystem=xdg-data/umu:create` — this is the canonical host-shared-runtime pattern CrossHook will follow.
- Known Steam Deck gaming-mode issues: black-screen without Shader Pre-Caching; Steam overlay z-order; HDR+gamescope+Flatpak on SteamOS 3.7.13. All upstream.

**Technical Context**

- CrossHook dispatch architecture fits a umu swap cleanly via intra-builder branching. No new dispatch method needed.
- Prior PR #148 failed specifically because `PROTON_VERB=runinprefix` was not set on trainer invocations — confirmed by revert-commit diff inspection. Phase 1 lands this hygiene before any umu path activates, making the regression architecturally impossible to recreate.
- Pressure-vessel filesystem bindings are the second-hardest problem; Phase 2 plumbing ships env vars that are inert under direct Proton and activate correctly under umu.
- Gamescope PID capture mechanism is unaffected by umu insertion — umu wraps beneath gamescope, not around it.
- Approximately 20 `crosshook-core` tests assert `"$PROTON" run` in emitted commands and need split assertions (fallback branch + umu branch).

---

## GitHub issues

Issues created from this PRD on **yandy-r/crosshook** (2026-04-14). Duplicating the `research-to-issues` import for this document will open **duplicate** issues; edit this table instead of re-importing.

### Phase tracking

| Phase | Tracking issue                                                             | Linked work                                                                                                                                                                                                                                                                                                            |
| ----- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | [#254](https://github.com/yandy-r/crosshook/issues/254)                    | [#234](https://github.com/yandy-r/crosshook/issues/234)                                                                                                                                                                                                                                                                |
| 2     | [#255](https://github.com/yandy-r/crosshook/issues/255)                    | [#235](https://github.com/yandy-r/crosshook/issues/235)                                                                                                                                                                                                                                                                |
| 3     | [#256](https://github.com/yandy-r/crosshook/issues/256)                    | [#236](https://github.com/yandy-r/crosshook/issues/236), [#237](https://github.com/yandy-r/crosshook/issues/237), [#238](https://github.com/yandy-r/crosshook/issues/238), [#243](https://github.com/yandy-r/crosshook/issues/243); related [#244](https://github.com/yandy-r/crosshook/issues/244)                    |
| 4     | [#257](https://github.com/yandy-r/crosshook/issues/257)                    | [#239](https://github.com/yandy-r/crosshook/issues/239)                                                                                                                                                                                                                                                                |
| 5     | [#258](https://github.com/yandy-r/crosshook/issues/258)                    | [#240](https://github.com/yandy-r/crosshook/issues/240), [#242](https://github.com/yandy-r/crosshook/issues/242), [#245](https://github.com/yandy-r/crosshook/issues/245), [#246](https://github.com/yandy-r/crosshook/issues/246)                                                                                     |
| 5b    | (no separate tracker — ships under phase-5b branch)                        | Resolves [#242](https://github.com/yandy-r/crosshook/issues/242) (Flathub NOT published — keep distro commands), [#244](https://github.com/yandy-r/crosshook/issues/244) (watchdog exe-name fallback + structured tracing), [#245](https://github.com/yandy-r/crosshook/issues/245) (Steam Deck caveats in onboarding) |
| 6     | Closed / retired ([#259](https://github.com/yandy-r/crosshook/issues/259)) | Closed child issue [#241](https://github.com/yandy-r/crosshook/issues/241); direct Proton retained for unsupported `umu-run` titles                                                                                                                                                                                    |

Phase 1 tracker [#254](https://github.com/yandy-r/crosshook/issues/254) links the other phase trackers in a comment for navigation.

### Implementation (child issues)

| #   | Issue                                                                                                  | Topic                                                                                                       |
| --- | ------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| 234 | [Phase 1: PROTON_VERB hygiene](https://github.com/yandy-r/crosshook/issues/234)                        | Game/trainer `PROTON_VERB`, `env.rs`, tests                                                                 |
| 235 | [Phase 2: pressure-vessel allowlist](https://github.com/yandy-r/crosshook/issues/235)                  | `STEAM_COMPAT_*`, `PRESSURE_VESSEL_*`                                                                       |
| 236 | [Phase 3a: UmuPreference + TOML + umu_game_id](https://github.com/yandy-r/crosshook/issues/236)        | Settings + profile fields                                                                                   |
| 237 | [Phase 3b: umu branch + PROTONPATH + Steam opt-out](https://github.com/yandy-r/crosshook/issues/237)   | Builders, non-Steam only                                                                                    |
| 238 | [Phase 3c: tests / E2E concurrency](https://github.com/yandy-r/crosshook/issues/238)                   | Command-shape split, non-regression smoke                                                                   |
| 247 | [Phase 3b: HTTP umu-database resolver + SQLite cache](https://github.com/yandy-r/crosshook/issues/247) | HTTP fetch + ETag revalidation + `external_cache_entries`; promoted from Phase 7+; #251 closed as duplicate |
| 263 | [Phase 3b: umu-database CSV coverage UI warning](https://github.com/yandy-r/crosshook/issues/263)      | Amber chip + badge when `will_use_umu && csv_coverage === missing`                                          |
| 239 | [Phase 4: Auto + exported scripts](https://github.com/yandy-r/crosshook/issues/239)                    | Default-on Auto, `export/launcher.rs`                                                                       |
| 240 | [Phase 5: Flatpak + onboarding](https://github.com/yandy-r/crosshook/issues/240)                       | Manifest, install dialog, nag dismissal                                                                     |
| 241 | [Phase 6: remove direct proton path](https://github.com/yandy-r/crosshook/issues/241)                  | Closed; direct Proton retained for compatibility                                                            |

### Decisions, research, and follow-ups

| #   | Issue                                                                                                 | Notes                                                                    |
| --- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ |
| 242 | [Flathub `org.openwinecomponents.umu.umu-launcher`?](https://github.com/yandy-r/crosshook/issues/242) | Blocks Phase 5 one-click UX                                              |
| 243 | [PROTONPATH dirname vs tag](https://github.com/yandy-r/crosshook/issues/243)                          | GE-Proton path semantics                                                 |
| 244 | [Gamescope SIGTERM vs PV teardown](https://github.com/yandy-r/crosshook/issues/244)                   | Watchdog / container edge cases                                          |
| 245 | [Steam Deck gaming-mode docs](https://github.com/yandy-r/crosshook/issues/245)                        | Onboarding only, upstream bugs                                           |
| 246 | [Telemetry baseline (optional)](https://github.com/yandy-r/crosshook/issues/246)                      | Success-metric quantification                                            |
| 247 | [HTTP umu-database resolver + SQLite cache](https://github.com/yandy-r/crosshook/issues/247)          | Promoted to Phase 3b; implements full-CSV HTTP fetch + ETag revalidation |

### Deferred / out of scope

| #   | Issue                                                                              | Topic                                                     |
| --- | ---------------------------------------------------------------------------------- | --------------------------------------------------------- |
| 248 | [No Steam profile umu migration](https://github.com/yandy-r/crosshook/issues/248)  | Two PV containers (closed as NOT_PLANNED 2026-04-17)      |
| 249 | [Custom Proton “tinkerer” UX](https://github.com/yandy-r/crosshook/issues/249)     | Opt-out only in PRD                                       |
| 250 | [Non-x86_64](https://github.com/yandy-r/crosshook/issues/250)                      | umu arch scope                                            |
| 251 | [v1 HTTP GAMEID (see #247)](https://github.com/yandy-r/crosshook/issues/251)       | Closed as duplicate of #247 (2026-04-14)                  |
| 252 | [Bundle umu in Flatpak](https://github.com/yandy-r/crosshook/issues/252)           | Host-shared preferred (closed as NOT_PLANNED 2026-04-16)  |
| 253 | [winetricks/protontricks via umu](https://github.com/yandy-r/crosshook/issues/253) | Wine-native, unchanged (closed as NOT_PLANNED 2026-04-17) |

---

_Generated: 2026-04-14_
_Status: DRAFT — needs validation_
