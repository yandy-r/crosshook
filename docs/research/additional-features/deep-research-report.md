# CrossHook Additional Features: Deep Research Report

**Date**: 2026-03-26
**Methodology**: Asymmetric Research Squad (8 specialized personas + 2 cross-analysis agents)
**Scope**: UI/UX enhancements, technical opportunities, additional scope for CrossHook

---

## Executive Summary

Eight research perspectives (Historical, Contrarian, Analogical, Systems, Journalistic, Archaeological, Futurist, Negative-Space) independently analyzed CrossHook's feature landscape. Two cross-analysis agents synthesized their findings into convergence maps and priority matrices.

**The meta-insight**: CrossHook's competitive advantage is not about adding more features -- it is about making the existing trainer-on-Linux workflow **reliable, diagnosable, and shareable**. Every perspective, from every angle, converges on the same conclusion: invest in depth over breadth.

**The single highest-value feature**: Trainer version / game version correlation -- recommended by 7/8 perspectives, fills a gap that NO other tool in the Linux gaming ecosystem addresses.

---

## Part 1: Priority Matrix

### P0 -- Must Have Next

| Feature                                        | Perspectives | Effort | Impact   | Codebase Ready |
| ---------------------------------------------- | :----------: | ------ | -------- | :------------: |
| Post-launch failure diagnostics                |     6/8      | Medium | Critical |    Partial     |
| Trainer onboarding / acquisition guidance      |     6/8      | Low    | Critical |    Partial     |
| Profile health dashboard / staleness detection |     6/8      | Medium | High     |     Ready      |
| Actionable validation error help text          |     5/8      | Low    | High     |     Ready      |
| Dry run / preview launch mode                  |     5/8      | Low    | High     |     Ready      |

**Why these are P0**: They address the most critical user journey failures. Users cannot debug launch failures, don't know where to get trainers, get unhelpful errors, can't preview what will happen, and don't know when profiles break. All have significant existing infrastructure (`validate()`, `resolve_launch_directives()`, `ProfileStore::list/load`).

#### 1. Post-Launch Failure Diagnostics

The console view streams raw log lines but provides no structured interpretation. When a launch fails:

- **Exit code analysis**: Translate signal codes (134=SIGABRT, 139=SIGSEGV, 137=SIGKILL/OOM) into human messages
- **Proton error pattern detection**: Recognize common WINE/Proton errors ("could not load ntdll.dll", "Bad EXE format") and provide actionable suggestions
- **Top 10 failure mode detection**: Version mismatch, missing .NET in prefix, wrong WINEPREFIX, timing issues, missing vcredist, architecture mismatch, file permission issues, Flatpak sandbox, anti-cheat interference, trainer version mismatch
- **Crash report collection**: Check `$STEAM_COMPAT_DATA_PATH/crashreports/` after non-zero exit

_Sources: Contrarian, Systems, Negative-Space, Journalistic, Archaeological, Futurist_

#### 2. Trainer Onboarding & Acquisition Guidance

The quickstart says "browse for the trainer executable" but never explains WHERE to get one. This is the single largest gap in the user journey. Needs:

- In-app guidance on trusted trainer sources (FLiNG, etc.)
- Explanation of which trainer types need which `TrainerLoadingMode` (SourceDirectory vs CopyToPrefix)
- First-run readiness check: "You're ready" vs "You need to launch this game through Steam first"
- Guided workflow chaining auto-populate + profile creation + launch

_Sources: Negative-Space, Contrarian, Journalistic, Analogical, Historical, Archaeological_

#### 3. Profile Health Dashboard

Profiles store static absolute paths. When games update, Proton versions change, or trainers are removed, profiles break silently. Needs:

- Batch `validate()` across all saved profiles
- "Healthy / Stale / Broken" status per profile using the existing compatibility badge pattern
- Proton path existence checking with suggestion to re-resolve
- Trainer file existence verification
- Display in sidebar or dedicated view

_Infrastructure exists_: `ProfileStore::list/load`, `validate()`, `CompatibilityBadge` component pattern.

_Sources: Negative-Space, Analogical, Contrarian, Futurist, Archaeological, Systems_

#### 4. Actionable Validation Errors

Current `ValidationError::message()` says WHAT is wrong but not WHY or HOW to fix it. Example: "The Steam compatdata path does not exist" should add "This usually means the game hasn't been launched through Steam yet. Launch it once through Steam to create the compatibility data."

- Add `help` field to each `ValidationError` variant
- Add severity level (fatal vs warning vs informational)
- Low effort: pure string additions to existing enum

_Sources: Negative-Space, Contrarian, Analogical, Archaeological, Systems_

#### 5. Dry Run / Preview Launch

A "Preview Launch" mode showing exactly what will happen without launching:

- Resolved environment variables from `resolve_launch_directives()`
- Wrapper command chain
- Effective `%command%` string from `build_steam_launch_options_command()`
- Validation results
- All computation functions already exist and are side-effect-free

_Infrastructure exists_: All functions are pure; just needs a UI surface.

_Sources: Analogical (Terraform plan), Contrarian, Archaeological, Negative-Space, Systems_

---

### P1 -- Should Have Soon

| Feature                                         | Perspectives | Effort  | Impact | Codebase Ready |
| ----------------------------------------------- | :----------: | ------- | ------ | :------------: |
| Trainer/game version correlation                |     7/8      | Medium  | High   |    Partial     |
| Profile override layers (portable base + local) |     5/8      | Medium  | High   |     Ready      |
| CLI completion (wire up placeholders)           |     4/8      | Low     | High   |     Ready      |
| Offline-first trainer management                |     5/8      | Medium  | High   |    Partial     |
| Community profile import wizard                 |     5/8      | Medium  | High   |     Ready      |
| Configuration history / diff / rollback         |     5/8      | Medium  | High   |    Partial     |
| Pinned profiles / favorites                     |     4/8      | Low     | High   |     Ready      |
| Proton version migration tool                   |     4/8      | Low-Med | High   |    Partial     |
| Diagnostic bundle export                        |     4/8      | Low     | Medium |    Partial     |

#### 6. Trainer/Game Version Correlation (Highest Cross-Perspective Support)

The single most recommended feature across all research. No tool in the Linux gaming ecosystem tracks the relationship between game versions and trainer versions.

- Detect game updates via Steam manifest timestamp changes
- Compare against profile's trainer version metadata
- Alert: "Game X updated. Your trainer may be incompatible."
- Track which trainer versions work with which game versions in community profiles
- `CommunityProfileMetadata` already has `game_version` and `trainer_version` fields -- currently display-only

_Sources: Contrarian, Journalistic, Negative-Space, Historical, Systems, Futurist, Analogical_

#### 7. Profile Override Layers

The biggest friction with community profiles: paths are machine-specific. Solution (from Docker Compose analogy):

- Split profiles into portable base (launch method, optimizations, trainer type) + local override (game path, prefix path, Proton path)
- `GameProfile`'s section-based design (`GameSection`, `TrainerSection`, `SteamSection`, `RuntimeSection`, `LaunchSection`) already supports selective merging
- Community profiles become path-portable templates

_Sources: Analogical, Systems, Contrarian, Negative-Space, Futurist_

#### 8. CLI Completion

6 of 7 CLI commands are placeholders returning "not_implemented". All business logic already exists in `crosshook-core`. Pure wiring work that unlocks headless/scripted usage, CI integration, and Steam Deck console-mode workflows.

_Sources: Archaeological, Contrarian, Futurist, Analogical_

#### 9. Configuration History / Rollback

"This game worked last Tuesday but doesn't work today. What changed?" -- unanswerable in every Linux launcher. TOML profiles are version-control-friendly. Track last 5 known-working configurations per profile.

_Sources: Contrarian, Analogical, Historical, Archaeological, Negative-Space_

---

### P2 -- Plan For

| Feature                                          | Perspectives | Effort  | Impact | Codebase Ready |
| ------------------------------------------------ | :----------: | ------- | ------ | :------------: |
| Optimization presets per profile (A/B configs)   |     4/8      | Medium  | Medium |     Ready      |
| Gamescope wrapper integration                    |     3/8      | Low-Med | Medium |    Partial     |
| Game metadata / cover art (SteamGridDB)          |     3/8      | Medium  | Medium |    Partial     |
| ProtonDB compatibility lookup                    |     4/8      | Medium  | Medium |    Partial     |
| Adaptive Deck Mode layout (CSS-driven)           |     3/8      | Low     | Medium |     Ready      |
| Community profile export from GUI                |     3/8      | Low     | Medium |     Ready      |
| Profile duplicate / clone                        |     3/8      | Low     | Medium |     Ready      |
| Custom env variables per profile                 |     3/8      | Low-Med | Medium |    Partial     |
| Extended optimization catalog (DXVK_ASYNC, etc.) |     3/8      | Low     | Medium |     Ready      |
| Tap pinning / version locking                    |     3/8      | Low     | Medium |     Ready      |
| Settings expansion (defaults, theme, log level)  |     3/8      | Low-Med | Medium |     Ready      |
| Prefix health monitoring / disk usage            |     3/8      | Medium  | Medium |    Partial     |
| Network isolation for trainers (unshare --net)   |     2/8      | Low     | Medium |    Partial     |
| Trainer hash verification (SHA-256)              |     2/8      | Low     | Medium |      None      |
| Stale launcher detection (is_stale field)        |     2/8      | Low     | Medium |     Ready      |
| MangoHud per-profile configuration               |     2/8      | Low-Med | Medium |    Partial     |
| Data-driven optimization catalog (loadable TOML) |     2/8      | Medium  | Medium |    Partial     |

**Quick wins in P2** (low effort + infrastructure exists):

- Profile clone: one Tauri command
- Community export button: backend function exists, needs Tauri command + UI button
- Stale launcher detection: flip `is_stale` from always-false to real
- Adaptive Deck layout: CSS-only via existing `data-crosshook-controller-mode` attribute
- Extended optimization catalog: add entries to existing `LAUNCH_OPTIMIZATION_DEFINITIONS`
- Tap pinning: add `pinned_commit` field to `CommunityTapSubscription`

---

### P3 -- Consider Later

| Feature                                   | Perspectives | Effort    | Impact |
| ----------------------------------------- | :----------: | --------- | ------ |
| Trainer discovery / search integration    |     3/8      | Very High | High   |
| Protontricks / winetricks integration     |     3/8      | High      | Medium |
| Flatpak distribution target               |     3/8      | High      | Medium |
| ProtonUp-Qt integration                   |     2/8      | High      | Medium |
| macOS port (GPTK2)                        |     2/8      | Very High | Medium |
| Lutris profile import                     |     2/8      | Medium    | Medium |
| Mod management (Nexus/Thunderstore)       |     2/8      | Very High | Medium |
| Profile collections / playlists           |     2/8      | Medium    | Low    |
| Launch pipeline visualization             |     2/8      | High      | Medium |
| Accessibility (ARIA, high contrast, Orca) |     2/8      | Medium    | Low    |
| ML-assisted configuration                 |     2/8      | Very High | Low    |

---

### Anti-Patterns -- Avoid

| Anti-Pattern                               | Warnings | Why                                                                                |
| ------------------------------------------ | :------: | ---------------------------------------------------------------------------------- |
| **Universal launcher (Epic/GOG/Amazon)**   |   5/8    | Scope creep is the #1 project killer. Trainer orchestration is the differentiator. |
| **Feature bloat / kitchen-sink**           |   5/8    | Every feature is a maintenance commitment. Subtract before adding.                 |
| **Social features / accounts / telemetry** |   4/8    | Linux community is privacy-first. Lutris accounts were largely ignored.            |
| **Library management as primary UX**       |   4/8    | Profile-based approach is correct. Users manage 5-10 profiles, not 500 games.      |
| **UI polish over core reliability**        |   3/8    | Diagnostics > visual features. A tool that explains failures beats a pretty one.   |
| **Forced auto-updates**                    |   3/8    | Users demand control. Changing WINE configs mid-session is unacceptable.           |
| **Trainer download marketplace**           |   3/8    | Legal liability, security concerns, maintenance burden. Guide, don't host.         |

---

## Part 2: Key Technical Opportunities

### Systems-Level Enhancements

#### Extended Proton Environment Variables

The optimization framework supports adding new entries with minimal code. High-value additions not yet in the catalog:

| Variable                             | Purpose                                        | Impact    |
| ------------------------------------ | ---------------------------------------------- | --------- |
| `DXVK_ASYNC=1`                       | Async shader compilation (prevents stuttering) | Very High |
| `DXVK_FRAME_RATE=N`                  | Frame rate cap (battery life on Deck)          | High      |
| `PROTON_NO_ESYNC=1`                  | Disable esync (compatibility fix)              | High      |
| `PROTON_NO_FSYNC=1`                  | Disable fsync (compatibility fix)              | High      |
| `PROTON_ENABLE_NVAPI=1`              | Enable NVIDIA API (DLSS support)               | High      |
| `PROTON_FORCE_LARGE_ADDRESS_AWARE=1` | Fix memory-limited 32-bit games                | Medium    |
| `PROTON_LOG=1`                       | Enable debug logging                           | Medium    |
| `VKD3D_CONFIG=dxr`                   | Ray tracing support                            | Medium    |

#### Process Monitoring via procfs

Replace `pgrep -x` with direct `/proc/<pid>/status` monitoring:

- Process state detection (running, zombie, dead)
- RSS memory tracking
- Exit code analysis with signal translation
- Process tree walking for WINE process discovery

#### Security: Network Isolation for Trainers

Highest-value, lowest-complexity security measure: `unshare --net` prevents trainers from making network connections (telemetry, update checks). Add as a per-profile wrapper toggle.

### Codebase Gaps Found (Archaeological)

| Gap                                                                         | Impact | Status                                                      |
| --------------------------------------------------------------------------- | ------ | ----------------------------------------------------------- |
| CLI: 6/7 commands are placeholders                                          | High   | Business logic exists in crosshook-core                     |
| `trainer.type` field: defined but never used                                | Medium | No behavior branching on trainer type                       |
| Injection system: data model exists, no runtime                             | High   | `dll_paths`/`inject_on_launch` stored, never evaluated      |
| `LaunchPhase`: 5-state lifecycle, game/trainer launches are fire-and-forget | High   | `WaitingForTrainer` implies coordination that doesn't exist |
| `is_stale` on `LauncherInfo`: always returns false                          | Medium | Detection logic not implemented                             |
| Settings model: only 3 fields                                               | Medium | Missing theme, default Proton, log level, etc.              |
| No `profile_duplicate` command                                              | Low    | Requires load + save-under-new-name                         |
| `ProfileData` type: orphaned TypeScript type                                | Low    | Legacy bridge type, never used                              |

---

## Part 3: Strategic Insights

### CrossHook's Maturity Position

The Historical perspective identified a universal 5-phase maturity progression for tools in this space:

| Phase                         | Description                               | CrossHook Status                                      |
| ----------------------------- | ----------------------------------------- | ----------------------------------------------------- |
| **1. Core Function**          | Solve one problem well                    | **Current** -- launch orchestration + trainer loading |
| **2. Automation & Discovery** | Auto-detect, templates, community configs | **Partial** -- auto-populate, community taps exist    |
| **3. Ecosystem Integration**  | External services, plugins, metadata      | **Not started**                                       |
| **4. Community & Curation**   | Ratings, verified configs, marketplace    | **Not started**                                       |
| **5. Platform Maturity**      | Themes, analytics, cross-platform         | **Not started**                                       |

**Recommendation**: Complete Phase 2 before entering Phase 3. The P0 and P1 features above are all Phase 2 completion items.

### Competitive Position

CrossHook occupies a genuinely unique market position. No other tool specifically orchestrates trainer launches on Linux:

| Tool        | Overlap                        | CrossHook Differentiator                           |
| ----------- | ------------------------------ | -------------------------------------------------- |
| Lutris      | Prefix management, WINE config | Trainer-specific orchestration, community profiles |
| Bottles     | Prefix management              | Trainer loading modes, launch optimizations        |
| Heroic      | Non-Steam game launching       | Trainer focus, Steam-first integration             |
| ProtonUp-Qt | Proton version management      | Built on top of this; complementary                |
| BoilR       | Adding non-Steam games         | Complementary; different problem                   |
| WeMod       | Trainer management             | Linux-native, open-source, offline-first           |

### The Contrarian's Most Important Insight

> "Make the exported launcher the primary product. Users configure once, export, and never open CrossHook again until something changes. This is the right UX, not a limitation."

The launcher export (.sh/.desktop) is under-appreciated as a differentiator. Consider extending exports to:

- Sunshine-compatible app configurations
- Headless CLI launch for automation
- Version-controlled shell scripts that capture the complete launch recipe

### Sustainability Warning

5/8 perspectives warn about solo maintainer burnout as the #1 project killer in this space. Mitigation strategies:

1. Keep the maintenance surface small (avoid scope creep)
2. Make integrations defensive (parse VDF loosely, fail gracefully)
3. Move compile-time constants to loadable data (optimization catalog)
4. Invest in automation (CI testing, profile validation tooling)
5. Use community taps to distribute maintenance (profiles, not code)

---

## Part 4: Effort vs. Impact Quick Reference

```
                        HIGH IMPACT
                            |
     P0: Diagnostics        |   P1: Version correlation
     P0: Onboarding         |   P1: Override layers
     P0: Dry run            |   P1: Config history
     P0: Validation help    |   P1: Import wizard
     P0: Health dashboard   |   P1: CLI completion
                            |
 LOW EFFORT ----------------+---------------- HIGH EFFORT
                            |
     P2: Profile clone      |   P3: Trainer discovery
     P2: Community export   |   P3: Mod management
     P2: Deck layout        |   P3: macOS port
     P2: Stale launchers    |   P3: ML configuration
     P2: Tap pinning        |   P3: Flatpak packaging
                            |
                        LOW IMPACT
```

---

## Research Methodology

### Phase 1: 8 Asymmetric Research Personas

| Persona        | Focus                                       | Key Contribution                                                                           |
| -------------- | ------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Historical     | Tool evolution patterns over 20 years       | 5-phase maturity model; community contribution flywheel                                    |
| Contrarian     | Challenge assumptions, find anti-patterns   | Scope discipline; exported launcher as primary product                                     |
| Analogical     | Patterns from 6 adjacent domains            | Profile overrides (Docker), dry run (Terraform), presets (DAW)                             |
| Systems        | Linux internals and Proton ecosystem        | 20+ env variables, procfs monitoring, security model                                       |
| Journalistic   | Current ecosystem state and community needs | WeMod/FLiNG Linux status, competitive landscape, top 5 needs                               |
| Archaeological | Hidden gaps in the CrossHook codebase       | 10 stub features, 7 missing modules, type system hints                                     |
| Futurist       | Emerging trends 2025-2027                   | NTSync, Wayland, WOW64, immutable distros, GPTK2                                           |
| Negative-Space | What nobody talks about but users need      | 6 critical gaps (onboarding, troubleshooting, maintenance, discovery, social, integration) |

### Phase 2: The Crucible (Cross-Analysis)

- **Convergence Analyst**: Identified 5 universal convergences, 6 strong convergences, and 4 contradictions/tensions
- **Strategic Priority Analyst**: Scored every feature on 5 dimensions, produced P0-P3 priority tiers

### Confidence Levels

- Historical patterns and codebase analysis: **High** (directly observable)
- Community needs and competitive landscape: **Medium-High** (based on knowledge through May 2025)
- Emerging trends and future predictions: **Medium** (based on trajectories, not confirmed roadmaps)
- Live community sentiment: **Medium** (web research tools were partially unavailable; analysis uses training knowledge)

---

## Appendix: Raw Research Files

Individual persona research outputs are available at:

- `docs/plans/futurist-perspective/research-emerging-trends.md` (committed to repo)
- Agent output files in `/tmp/claude-1000/` (session-local, not persisted)
