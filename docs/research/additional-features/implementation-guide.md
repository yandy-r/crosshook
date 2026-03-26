# CrossHook Feature Implementation Guide

**Tracking issue**: #78
**Research source**: docs/research/additional-features/deep-research-report.md
**Last updated**: 2026-03-26

This document provides a recommended implementation order, dependency map, and quick-win guide for the features identified in the deep research analysis. Use it alongside the GitHub issues to plan sprints.

---

## Guiding Principle

> Invest in depth over breadth. Every feature should make trainer orchestration more reliable, diagnosable, or shareable -- not expand CrossHook into a general-purpose launcher.

---

## Quick Wins

These features require minimal effort because the infrastructure already exists. They can be completed in a day or less each and provide immediate user-facing value.

| #   | Feature                       | Status | Effort | What To Do                                                                                | Key Files                                                                                                                                          |
| --- | ----------------------------- | :----: | ------ | ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| #39 | Actionable validation errors  |  Done  | Hours  | Shipped structured launch validation help/severity metadata and LaunchPanel guidance UI   | `crates/crosshook-core/src/launch/request.rs`, `src-tauri/src/commands/launch.rs`, `src/hooks/useLaunchState.ts`, `src/components/LaunchPanel.tsx` |
| #56 | Profile duplicate / clone     |        | Hours  | Add Tauri command: `load(name)` + `save(new_name)` with conflict check                    | `src-tauri/src/commands/profile.rs`, `ProfileActions.tsx`                                                                                          |
| #55 | Community profile export      |        | Hours  | Add Tauri command wrapping existing `export_community_profile()` + UI button              | `src-tauri/src/commands/community.rs`                                                                                                              |
| #64 | Stale launcher detection      |        | Hours  | Implement real `is_stale` logic: compare launcher paths vs current profile                | `crates/crosshook-core/src/export/launcher_store.rs`                                                                                               |
| #54 | Adaptive Deck Mode layout     |        | Hours  | CSS custom properties keyed on `data-crosshook-controller-mode` attribute                 | `src/styles/variables.css`, `src/styles/theme.css`                                                                                                 |
| #59 | Tap pinning                   |        | Hours  | Add `pinned_commit: Option<String>` to `CommunityTapSubscription`, gate `fetch_and_reset` | `crates/crosshook-core/src/community/taps.rs`                                                                                                      |
| #58 | Extended optimization catalog |        | Hours  | Add 8 new entries to `LAUNCH_OPTIMIZATION_DEFINITIONS` array                              | `crates/crosshook-core/src/launch/optimizations.rs`                                                                                                |

**Recommended approach**: Batch these into a single sprint or PR series. Each is independently shippable.

---

## Recommended Implementation Order

Features are ordered by dependency chains, progressive value delivery, and effort sequencing. Complete each phase before moving to the next.

### Phase 1: Foundation (Error Communication)

_Goal: Users understand what happened when something fails._

```
 #39 Actionable validation errors   ──┐
                                      ├──> Phase 1 complete
 #40 Dry run / preview launch       ──┘
```

| Order | Issue                                   | Status | Rationale                                                                                                                                                              |
| :---: | --------------------------------------- | :----: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
|   1   | **#39** -- Actionable validation errors |  Done  | Lowest effort P0. Immediate UX improvement. Sets the pattern for all error communication.                                                                              |
|   2   | **#40** -- Dry run / preview launch     |        | All computation functions are pure and side-effect-free. Wire `validate()` + `resolve_launch_directives()` + `build_steam_launch_options_command()` into a preview UI. |

**Dependencies**: None. These are standalone improvements.
**Estimated effort**: 2-3 days total.

---

### Phase 2: Diagnostics & Health (Reliability Layer)

_Goal: Users know when things are broken and why._

```
 #39 (Phase 1) ──> #36 Post-launch diagnostics ──┐
                                                   ├──> #49 Diagnostic bundle
 #38 Profile health dashboard ────────────────────┘
```

| Order | Issue                                      | Status | Rationale                                                                                                                              |
| :---: | ------------------------------------------ | :----: | -------------------------------------------------------------------------------------------------------------------------------------- |
|   3   | **#36** -- Post-launch failure diagnostics |        | Builds on #39's error communication pattern. Adds exit code analysis, Proton error detection, crash report collection.                 |
|   4   | **#38** -- Profile health dashboard        |        | Batch `validate()` across all profiles. Reuses the validation help text from #39. Surface health in sidebar.                           |
|   5   | **#49** -- Diagnostic bundle export        |        | Combines outputs from #36 (launch logs) and #38 (profile health) into a shareable archive. Natural capstone for the diagnostics phase. |

**Dependencies**: #39 should be done first (establishes error patterns). #36 and #38 can run in parallel.
**Estimated effort**: 1-2 weeks total.

---

### Phase 3: Profile Infrastructure (Core Improvements)

_Goal: Profiles are robust, portable, and easy to manage._

```
 Quick wins: #56 clone, #55 export, #64 stale launchers
                                                          ──> #42 Override layers ──> #45 Import wizard
 #47 Pinned profiles / favorites
 #48 Proton version migration
```

| Order | Issue                                  | Status | Rationale                                                                                                                                 |
| :---: | -------------------------------------- | :----: | ----------------------------------------------------------------------------------------------------------------------------------------- |
|   6   | **#56** -- Profile clone               |        | Quick win. Unblocks #50 (optimization presets need easy profile variants).                                                                |
|   7   | **#55** -- Community export from GUI   |        | Quick win. Backend function exists.                                                                                                       |
|   8   | **#64** -- Stale launcher detection    |        | Quick win. Flip `is_stale` from always-false to real comparison.                                                                          |
|   9   | **#47** -- Pinned profiles / favorites |        | Small `AppSettingsData` extension. High UX value on Steam Deck.                                                                           |
|  10   | **#48** -- Proton migration tool       |        | Detect stale Proton paths, suggest replacements from discovery. Natural extension of #38 health dashboard.                                |
|  11   | **#42** -- Profile override layers     |        | The biggest single improvement for community profile adoption. Split portable base from local paths.                                      |
|  12   | **#45** -- Import wizard               |        | Depends on #42 (override layers) to properly separate portable and local concerns during import. Orchestrates auto-populate + validation. |

**Dependencies**: #42 should precede #45. Quick wins (#56, #55, #64) have no dependencies.
**Estimated effort**: 2-3 weeks total.

---

### Phase 4: Version Intelligence (Competitive Differentiator)

_Goal: CrossHook understands version relationships -- the gap no other tool fills._

```
 #41 Trainer/game version correlation ──> #46 Configuration history
                                     ──> #37 Onboarding guidance
```

| Order | Issue                                       | Status | Rationale                                                                                                                                                     |
| :---: | ------------------------------------------- | :----: | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
|  13   | **#41** -- Trainer/game version correlation |        | Highest cross-perspective support (7/8). Detect game updates via Steam manifest changes, warn about trainer compatibility.                                    |
|  14   | **#46** -- Configuration history / rollback |        | Builds on version awareness. Track which configs worked, enable diff and rollback.                                                                            |
|  15   | **#37** -- Trainer onboarding guidance      |        | With version tracking (#41), health dashboard (#38), and diagnostics (#36) in place, the onboarding flow can guide users through a complete, validated setup. |

**Dependencies**: #41 depends on Steam manifest parsing (exists). #46 builds on #41's version awareness. #37 benefits from #38, #36, and #41 being complete.
**Estimated effort**: 2-3 weeks total.

---

### Phase 5: CLI & Automation (Power Users)

_Goal: CrossHook is usable headlessly, scriptable, and automatable._

```
 #43 CLI completion ──> #44 Offline-first
                   ──> headless launch for Sunshine
```

| Order | Issue                                       | Status | Rationale                                                                                               |
| :---: | ------------------------------------------- | :----: | ------------------------------------------------------------------------------------------------------- |
|  16   | **#43** -- CLI completion                   |        | Pure wiring to `crosshook-core`. Unlocks scripted usage, automation, Steam Deck console mode.           |
|  17   | **#44** -- Offline-first trainer management |        | With CLI complete, ensure all workflows function without network. Critical for Steam Deck portable use. |

**Dependencies**: #43 is standalone wiring work. #44 is a cross-cutting concern that should be validated after CLI exists.
**Estimated effort**: 1-2 weeks total.

---

### Phase 6: Polish & Ecosystem (P2 Features)

_Goal: Enhance the experience for established users. Pick based on demand._

These features have no strict ordering. Prioritize based on community feedback.

| Issue | Category                         | Status | Effort  | Good Pairing With         |
| ----- | -------------------------------- | :----: | ------- | ------------------------- |
| #58   | Extended optimization catalog    |        | Low     | #66 (data-driven catalog) |
| #59   | Tap pinning                      |        | Low     | #55 (community export)    |
| #54   | Adaptive Deck Mode layout        |        | Low     | #47 (pinned profiles)     |
| #50   | Optimization presets             |        | Medium  | #58 (extended catalog)    |
| #57   | Custom env vars per profile      |        | Low-Med | #58 (extended catalog)    |
| #51   | Gamescope wrapper                |        | Low-Med | #58 (extended catalog)    |
| #65   | MangoHud per-profile config      |        | Low-Med | #51 (gamescope)           |
| #53   | ProtonDB lookup                  |        | Medium  | #41 (version correlation) |
| #52   | Game metadata / cover art        |        | Medium  | #53 (ProtonDB)            |
| #60   | Settings expansion               |        | Low-Med | Any phase                 |
| #61   | Prefix health monitoring         |        | Medium  | #38 (health dashboard)    |
| #62   | Network isolation                |        | Low     | #63 (hash verification)   |
| #63   | Trainer hash verification        |        | Low     | #62 (network isolation)   |
| #66   | Data-driven optimization catalog |        | Medium  | #58 (extended catalog)    |

**Natural groupings for PRs**:

- **Launch optimization bundle**: #58 + #50 + #57 + #66
- **Steam Deck polish bundle**: #54 + #51 + #65
- **Security bundle**: #62 + #63
- **Ecosystem integration bundle**: #53 + #52
- **Community improvements bundle**: #59 + #55 (if not done earlier)
- **Settings & maintenance bundle**: #60 + #61

---

### Phase 7: Future (P3 Features)

These are tracked but not scheduled. Revisit after Phases 1-6 based on community demand and maintainer capacity.

| Issue                           | Status | Trigger to Revisit                                                               |
| ------------------------------- | :----: | -------------------------------------------------------------------------------- |
| #67 -- Trainer discovery        |        | When community taps reach 50+ profiles and users still struggle to find trainers |
| #68 -- Protontricks integration |        | When trainer failure diagnostics (#36) show missing dependencies as a top cause  |
| #69 -- Flatpak distribution     |        | When immutable distro users report AppImage issues                               |
| #70 -- ProtonUp-Qt integration  |        | When Proton migration (#48) shows users lack the needed Proton versions          |
| #71 -- Lutris import            |        | When user acquisition from Lutris becomes a measurable source                    |
| #72 -- Mod management           |        | Only if directly supporting trainer coexistence, not as general mod management   |
| #73 -- Profile collections      |        | When users have 20+ profiles and request organization                            |
| #74 -- Pipeline visualization   |        | When the profile editor feels too complex for new users                          |
| #75 -- Accessibility            |        | When accessibility feedback is received or before a public launch milestone      |
| #76 -- macOS port               |        | When GPTK2 trainer viability is confirmed by community testing                   |
| #77 -- ML configuration         |        | After #53 (ProtonDB) is live and generating data for pattern extraction          |

---

## Dependency Graph

```
Phase 1 (Foundation)
  #39 Validation errors ─────────────────────────────────────┐
  #40 Dry run ───────────────────────────────────────────────┤
                                                             │
Phase 2 (Diagnostics)                                        │
  #36 Post-launch diagnostics ◄──── #39                      │
  #38 Profile health dashboard                               │
  #49 Diagnostic bundle ◄──── #36 + #38                      │
                                                             │
Phase 3 (Profiles)                                           │
  #56 Clone (quick win)                                      │
  #55 Community export (quick win)                           │
  #64 Stale launchers (quick win)                            │
  #47 Pinned profiles                                        │
  #48 Proton migration ◄──── #38                             │
  #42 Override layers                                        │
  #45 Import wizard ◄──── #42                                │
                                                             │
Phase 4 (Version Intelligence)                               │
  #41 Version correlation                                    │
  #46 Config history ◄──── #41                               │
  #37 Onboarding ◄──── #38 + #36 + #41                      │
                                                             │
Phase 5 (CLI)                                                │
  #43 CLI completion                                         │
  #44 Offline-first ◄──── #43                                │
                                                             │
Phase 6 (Polish) ── pick based on demand ────────────────────┘
  #58 Extended catalog     #50 Presets        #62 Net isolation
  #59 Tap pinning          #57 Custom env     #63 Hash verify
  #54 Deck layout          #51 Gamescope      #66 Data catalog
  #53 ProtonDB             #65 MangoHud       #60 Settings
  #52 Cover art            #61 Prefix health
```

---

## Effort Estimates by Phase

| Phase                  | Issues | Status | Estimated Total | Cumulative |
| ---------------------- | :----: | :----: | :-------------: | :--------: |
| Quick Wins             |   7    |        |    2-3 days     |  2-3 days  |
| Phase 1: Foundation    |   2    |        |    2-3 days     |  ~1 week   |
| Phase 2: Diagnostics   |   3    |        |    1-2 weeks    |  ~3 weeks  |
| Phase 3: Profiles      |   7    |        |    2-3 weeks    |  ~6 weeks  |
| Phase 4: Version Intel |   3    |        |    2-3 weeks    |  ~9 weeks  |
| Phase 5: CLI           |   2    |        |    1-2 weeks    | ~11 weeks  |
| Phase 6: Polish        |   17   |        |  Pick & choose  |  Ongoing   |
| Phase 7: Future        |   11   |        |  Not scheduled  |  Backlog   |

---

## Anti-Pattern Checklist

Before starting any feature, verify it does not fall into a warned pattern:

- [ ] Does this make **trainer management** better, or does it make CrossHook more like Lutris?
- [ ] Is the maintenance burden proportional to the user value?
- [ ] Does it work **offline** on Steam Deck?
- [ ] Does it respect user **privacy** (no telemetry, no accounts, no tracking)?
- [ ] Would a **solo maintainer** be able to keep this working for 3 years?
- [ ] Is this the **simplest** solution, or am I over-engineering?

If any answer raises doubt, reconsider or scope down before implementing.
