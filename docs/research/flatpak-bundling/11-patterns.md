# Cross-Cutting Pattern Synthesis

**Perspective**: Pattern Synthesizer
**Date**: 2026-04-15
**Scope**: Emergent patterns across all Phase 1 research (01-08) and Phase 2 crucible analysis (09-10)
**Input files**: All 10 research files in `docs/research/flatpak-bundling/`

---

## Executive Summary

Ten research files, eight perspectives, ~5,000 lines of analysis -- and five patterns emerge with force. The single most important finding is not about any specific tool: **CrossHook is an orchestrator, not an application, and this identity determines every downstream decision.** The research corpus converges on a clear answer once this framing is accepted. CrossHook should not bundle tools. It should invest in three things: (1) detection and guidance for missing host tools, (2) the D-Bus portal path for GameMode, and (3) a native Proton version download feature. Everything else is either technically impossible to bundle, architecturally irrelevant inside the sandbox, or unsustainable for a small team.

---

## 1. Recurring Themes

### Theme 1: The Orchestrator Identity (7 of 10 files)

The most consistent finding across the research: **CrossHook does not execute games. It tells the host to execute them.**

| File                | How it appears                                                                      |
| ------------------- | ----------------------------------------------------------------------------------- |
| `02-contrarian`     | "Orchestrators Don't Execute" -- bundling tools CrossHook never invokes is circular |
| `04-systems`        | Execution chain diagram shows every tool runs on host via `flatpak-spawn --host`    |
| `06-archaeological` | 13 external tools documented; all route through `platform.rs` host delegation       |
| `09-contradictions` | Identifies "application vs. orchestrator" as THE central strategic question         |
| `10-evidence`       | Rates architecture claims as highest-confidence evidence in the corpus              |
| `03-analogical`     | Notes Podman Desktop (thin GUI + host daemon) as better analogue than Bottles       |
| `08-negative-space` | Frames the question as "what runs where" rather than "what to package"              |

This is not a matter of opinion -- it is a structural fact about the codebase. `platform.rs` contains 1,400+ lines of abstraction specifically for bridging sandbox-to-host execution. The Flatpak manifest declares `--talk-name=org.freedesktop.Flatpak` because the app fundamentally cannot function without host process control.

**Pattern**: When 7 of 10 research perspectives independently arrive at the same architectural conclusion, the conclusion is likely correct.

### Theme 2: The Host-Context Problem (6 of 10 files)

Tools must run in the same context as the game process. Since games run on the host, tools must also be on the host.

| Tool               | Why it must be host-side                                             | Files      |
| ------------------ | -------------------------------------------------------------------- | ---------- |
| Winetricks/winecfg | Must use the exact Wine/Proton version that owns the game's prefix   | 01, 02, 04 |
| MangoHud           | Vulkan layer injected into the game process, which is a host process | 01, 04     |
| Gamescope          | Compositor that wraps the game window, which is a host window        | 01, 02, 04 |
| GameMode           | Daemon adjusts host CPU/GPU governors for the game's host PID        | 01, 04     |
| umu-launcher       | Manages Steam Linux Runtime containers for host-launched games       | 04, 05     |

A MangoHud installed inside CrossHook's Flatpak sandbox would overlay CrossHook's own Tauri window, not the game. A gamescope inside the sandbox would compose CrossHook's window, not the game window. A winetricks matching a different Wine version would corrupt the game prefix.

`10-evidence` confirms this is a high-confidence finding verified from source code and tool architecture.

**Pattern**: The sandbox boundary is a context boundary. Tools that interact with the game process must share the game's execution context (the host).

### Theme 3: The "Already Solved" Stack (5 of 10 files)

For several tools, the ecosystem already has the right integration mechanism -- CrossHook just needs to use it.

| Tool               | Existing Solution                                                   | Status               | Files          |
| ------------------ | ------------------------------------------------------------------- | -------------------- | -------------- |
| GameMode           | `org.freedesktop.portal.GameMode` D-Bus portal                      | Stable, version 4    | 01, 02, 04, 05 |
| MangoHud           | `MANGOHUD=1` env var threaded to host via `host_command_with_env()` | Working in CrossHook | 04, 06         |
| Gamescope          | Host binary invoked via `flatpak-spawn --host gamescope`            | Working in CrossHook | 04, 06         |
| Process management | Host `kill`/`ps`/`cat` via `host_std_command()`                     | Working in CrossHook | 06             |
| Proton/Wine        | Host Proton via `flatpak-spawn --host` with env threading           | Working in CrossHook | 04, 06         |

CrossHook's `platform.rs` already implements the correct abstraction for most tools. The code exists. The architecture is correct. The evidence assessment (`10-evidence`) rates these claims at highest confidence.

**Pattern**: The problem is less "how to integrate tools" and more "how to handle when tools are missing." The integration architecture is largely solved; the UX for missing dependencies is not.

### Theme 4: The Sustainability Constraint (4 of 10 files)

Even if bundling were technically correct, the maintenance burden may exceed a small team's capacity.

| Concern                                        | Evidence                                             | Files  |
| ---------------------------------------------- | ---------------------------------------------------- | ------ |
| 7+ independent release cycles to track         | Each bundled tool has its own upstream cadence       | 02, 08 |
| 240+ test scenarios for full bundling          | 6 tools x 8 host configs x 5 distros                 | 08     |
| Security patch delay (days-to-weeks vs. hours) | Flatpak ecosystem documents this pattern across CVEs | 02, 08 |
| No security response team                      | CrossHook is a small open-source project             | 08     |

`10-evidence` rates the "unsustainable for a small team" argument as Medium-Low because it assumes all-or-nothing bundling (a straw man). However, even selective bundling of 1-2 tools adds measurable ongoing cost. The sustainability constraint is a legitimate veto power.

`09-contradictions` identifies this as a "critical" severity item that blocks synthesis. The resolution: **don't propose bundling strategies that the team can't sustain.**

**Pattern**: Technical feasibility is necessary but not sufficient. Organizational capacity is the binding constraint for a small project.

### Theme 5: The Detect-Guide-Degrade Pattern (6 of 10 files)

The most actionable recommendation across the research is not about bundling at all -- it's about UX when tools are missing.

| File                | How it recommends this                                                                         |
| ------------------- | ---------------------------------------------------------------------------------------------- |
| `03-analogical`     | "Detect + Prompt + Install" is the universal pattern across macOS, Android, Podman Desktop     |
| `06-archaeological` | Documents CrossHook's existing `host_command_exists()` and graceful degradation chain          |
| `08-negative-space` | Identifies the first-run experience gap as the real battleground                               |
| `05-investigative`  | Notes Bottles v63+ added robust extension availability checks as a UX improvement              |
| `02-contrarian`     | Recommends: "Detect host tools at startup, report available/missing, provide install guidance" |
| `09-contradictions` | Lists "Can CrossHook detect and guide host tool installation?" as unresolved question #3       |

CrossHook already has detection infrastructure (`host_command_exists()`, readiness checks in `onboarding/readiness.rs`, distro-specific install advice in `build_umu_install_advice()`). The gap is in the UI layer: surfacing missing tools clearly, providing copy-to-clipboard install commands, and guiding first-time setup.

**Pattern**: The highest-ROI investment is not in packaging but in onboarding UX. A great setup wizard with host tools beats a mediocre auto-bundle.

---

## 2. Unexpected Connections

### Connection 1: Background Portal Is Not a Risk (Corrected)

`05-investigative` warns about `org.freedesktop.portal.Background` silently killing CrossHook-launched games. `10-evidence` identifies this as **likely incorrect**: games launch via `flatpak-spawn --host` and run as HOST processes. The Background portal monitors SANDBOX processes. The game PIDs are in the host namespace, invisible to the portal.

However, the Background portal IS relevant to CrossHook itself. If the user minimizes CrossHook while a game runs, xdg-desktop-portal could classify the Tauri window as "background" and terminate it. This would kill the watchdog that monitors the game process. **The risk is to CrossHook's own process, not to the game.**

This means CrossHook should call `RequestBackground` not to protect games, but to protect its own watchdog process.

### Connection 2: The VulkanLayer Extension Misdirection

`03-analogical` and `01-historical` recommend VulkanLayer extensions for MangoHud and gamescope. `04-systems` argues these are irrelevant because games run on the host. `09-contradictions` flags this as a high-severity contradiction.

The resolution is straightforward once the execution model is understood:

- VulkanLayer extensions inject into processes running INSIDE the Flatpak sandbox.
- CrossHook's games run OUTSIDE the sandbox (on the host).
- Therefore, VulkanLayer extensions would apply to CrossHook's own Tauri/WebKitGTK rendering, which is irrelevant.
- The correct integration for MangoHud and gamescope is what CrossHook already does: thread `MANGOHUD=1` as an env var and prepend `gamescope` as a command wrapper to the host-launched game process.

**VulkanLayer extensions are the wrong mechanism for CrossHook.** This is one of the research corpus's most important errors -- it's the most concrete "what to do" recommendation in Phase 1, and it's technically invalid for CrossHook's execution model.

### Connection 3: Proton Download Manager Is the Only Viable "Bundling"

`04-systems` identifies Proton version management as the sole viable integration (not bundling a tool, but implementing download/extract functionality natively). This aligns with `01-historical`'s documentation of Heroic's `heroic-wine-downloader` and Lutris's internal runner manager.

The connection: CrossHook could implement a Proton/GE-Proton download feature in Rust without external tool dependencies. This uses existing permissions (`--share=network`, `--filesystem=home`), doesn't require `flatpak-spawn --host`, and works identically in both AppImage and Flatpak. It's the one area where "bundling" (as a built-in feature) aligns with the orchestrator architecture rather than fighting it.

### Connection 4: The umu-launcher Convergence Simplifies Everything

`07-futurist` projects umu-launcher handling all Proton orchestration by 2027 (80% probability). `06-archaeological` documents CrossHook's existing umu integration with multi-strategy resolution. `04-systems` notes CrossHook already has `--filesystem=xdg-data/umu:create` for shared runtime data.

The connection: as umu-launcher matures, CrossHook's dependency surface _shrinks_. Instead of needing Proton, Steam Runtime, protonfixes, and Proton discovery independently, CrossHook delegates all of this to `umu-run`. The "install umu-run" step becomes the primary prerequisite, and umu itself handles downstream dependencies.

This convergence weakens the bundling argument further: why bundle tools that umu-launcher will manage?

### Connection 5: The Bazzite/SteamOS Pre-Installation Paradox

`07-futurist` argues immutable distros are the strongest bundling argument. `05-investigative` and `08-negative-space` document that gaming-focused immutable distros pre-install everything. `09-contradictions` elevates this to a high-severity item.

The paradox resolves when the user segments are distinguished:

- **Gaming immutable distros** (Bazzite, SteamOS, ChimeraOS): Tools pre-installed. Bundling is pure redundancy.
- **General immutable distros** (Fedora Atomic, NixOS): Tools NOT pre-installed. Bundling would help.
- **Traditional distros** (Arch, Fedora, Ubuntu): Tools easily installable. Bundling unnecessary.

The bundling argument applies to exactly one segment: general immutable distro users who want to game. This is a real population (growing, per `07-futurist`), but it's likely the smallest of the three segments among CrossHook's actual users. Without user demographics data (`10-evidence` Gap #1), the magnitude cannot be quantified.

---

## 3. Tool Tiering

Based on evidence quality ratings from `10-evidence`, architectural analysis from `04-systems` and `06-archaeological`, ecosystem precedents from `01-historical`, and contradiction resolution from `09-contradictions`:

### Tier 1: Should Definitely Delegate to Host (Do Not Bundle)

| Tool                             | Reason                                                                                                                         | Confidence  | CrossHook Status                                                                            |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ----------- | ------------------------------------------------------------------------------------------- |
| **Gamescope**                    | Compositor must wrap host game window; cannot meaningfully compose from inside sandbox; VulkanLayer extension is a known hack  | High        | Already works via `flatpak-spawn --host`                                                    |
| **Winetricks / Protontricks**    | Must match exact host Wine/Proton version; sandbox library conflicts documented; prefix corruption risk                        | High        | Already works via `flatpak-spawn --host`; detection chain in `prefix_deps/detection.rs`     |
| **Winecfg**                      | IS part of Wine; bundling means bundling all of Wine (~500MB+); version mismatch = prefix corruption                           | High        | Already works via `flatpak-spawn --host`                                                    |
| **MangoHud**                     | Vulkan layer must inject into host game process; env var threading (`MANGOHUD=1`) already works                                | High        | Already works via `host_command_with_env()`                                                 |
| **umu-launcher**                 | Python + Rust + Steam Runtime dependency chain is enormous; pre-built zipapp fails in Flatpak; host Steam interaction required | High        | Already works via `flatpak-spawn --host`; multi-strategy resolution in `runtime_helpers.rs` |
| **CachyOS kernel optimizations** | Kernel-level features (schedulers, sysctl, CPU flags); impossible to bundle by definition                                      | High        | Detection + display only                                                                    |
| **Wine Wayland driver**          | Part of Wine; cannot be separated; depends on host Wine/Proton build configuration                                             | Medium-High | Detection + env var toggle possible                                                         |

### Tier 2: Should Definitely Use Existing Mechanism (Already Solved)

| Tool                        | Mechanism                                                                                                                           | Confidence | CrossHook Status                                                                              |
| --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------- |
| **GameMode**                | `org.freedesktop.portal.GameMode` D-Bus portal (version 4); PID namespace translation handled by portal; host daemon does real work | High       | `gamemoderun` wrapper / `libgamemodeauto.so` preload already threaded via host launch command |
| **flatpak-spawn**           | Implicitly available in all Flatpak sandboxes; `is_flatpak()` detection cached via `OnceLock`                                       | High       | Fully implemented in `platform.rs`                                                            |
| **Host process management** | `kill`/`ps`/`cat`/`test` via `host_std_command()`                                                                                   | High       | Fully implemented in `watchdog.rs`                                                            |

### Tier 3: Should Build as Native Feature (Not "Bundling" -- Feature Development)

| Feature                                | Approach                                                                                                                    | Confidence  | CrossHook Status                                                                  |
| -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ----------- | --------------------------------------------------------------------------------- |
| **Proton version download/management** | Implement GitHub API client in Rust to download GE-Proton/CachyOS-Proton releases; extract to `compatibilitytools.d/`       | Medium-High | Not yet implemented; `protonup_binary_path` setting exists as placeholder         |
| **First-run onboarding wizard**        | Detect installed tools via `host_command_exists()`; display checklist with status; provide distro-specific install commands | High        | Partial: `onboarding/readiness.rs` has detection; UI presentation needs expansion |
| **Missing tool guidance**              | Grey out unavailable features with explanation and install instructions; copy-to-clipboard commands                         | High        | Partial: some degradation exists but guidance UX is minimal                       |

### Tier 4: Not Applicable / Not CrossHook's Problem

| Item                                    | Reason                                                                         | Confidence                     |
| --------------------------------------- | ------------------------------------------------------------------------------ | ------------------------------ |
| **Steam client**                        | Users install Steam themselves; CrossHook detects it; not a bundling candidate | High                           |
| **GPU drivers**                         | System-level; Flatpak runtime handles driver matching; not a bundling concern  | High                           |
| **Kernel features (NTSYNC, sched_ext)** | Kernel-level; impossible to influence from userspace app                       | High                           |
| **VulkanLayer Flatpak extensions**      | Irrelevant to CrossHook -- games run on host, not in sandbox                   | High (corrected from research) |

---

## 4. Minimum Viable Flatpak Experience (MVFE)

The smallest set of changes that gives the biggest UX improvement for CrossHook's Flatpak distribution, ranked by effort-vs-impact:

### MVFE Level 1: Zero Code Changes (Manifest + Docs Only)

| Change                                                                                 | Effort | Impact   | Rationale                                                          |
| -------------------------------------------------------------------------------------- | ------ | -------- | ------------------------------------------------------------------ |
| Verify `--talk-name=org.freedesktop.Flatpak` justification text for Flathub submission | Low    | Critical | Without this, CrossHook cannot be on Flathub at all                |
| Document prerequisite tools and distro-specific install commands                       | Low    | High     | Users need to know what to install before using CrossHook Flatpak  |
| Add `RequestBackground` portal call to prevent watchdog termination                    | Low    | Medium   | CrossHook's own process (not games) could be killed when minimized |

### MVFE Level 2: Detection + Guidance Improvements (Moderate Code)

| Change                                                                                            | Effort | Impact | Rationale                                                                             |
| ------------------------------------------------------------------------------------------------- | ------ | ------ | ------------------------------------------------------------------------------------- |
| Expand onboarding readiness checks to cover ALL tools (gamescope, MangoHud, winetricks, GameMode) | Medium | High   | Currently checks Steam, Proton, umu-run; should check all optional tools              |
| Add distro-family detection to provide targeted install commands                                  | Medium | High   | `HostDistroFamily` enum exists in `readiness.rs`; extend with install command mapping |
| Surface missing tools in the UI with clear explanations and copy-to-clipboard commands            | Medium | High   | Replace silent degradation with visible guidance                                      |
| Grey out optimization toggles when dependencies are missing (with "install X to enable" text)     | Medium | Medium | Prevents user confusion about why features don't work                                 |

### MVFE Level 3: Feature Development (Significant Code)

| Change                                                                                            | Effort | Impact | Rationale                                                                                 |
| ------------------------------------------------------------------------------------------------- | ------ | ------ | ----------------------------------------------------------------------------------------- |
| Build native Proton version download/management (GE-Proton, CachyOS-Proton)                       | High   | High   | Eliminates the external ProtonUp-Qt dependency; works identically in AppImage and Flatpak |
| Implement `RequestBackground` portal integration                                                  | Medium | Medium | Protects CrossHook's watchdog process during game sessions                                |
| Add Flatpak Steam write access negotiation (upgrade `:ro` to `:rw` for compat tools installation) | Low    | Medium | Currently read-only for Flatpak Steam; needed if CrossHook manages Proton versions        |

### What NOT to Do

| Non-action                                                     | Why                                                                     | Source                               |
| -------------------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------ |
| Do not bundle Wine/Proton inside the Flatpak                   | Architectural mismatch; massive size; version conflict with host        | 01, 02, 04, 10                       |
| Do not add VulkanLayer extensions to the manifest              | Games run on host; extensions inject into sandbox processes; irrelevant | 04, 09, 10                           |
| Do not implement a "host tool fallback" dual-path architecture | Testing matrix explosion; "uncanny valley" UX; maintenance burden       | 08, 09                               |
| Do not bundle gamescope or MangoHud                            | Must run in host context; sandbox versions cannot reach game processes  | 04, 06, 10                           |
| Do not attempt to install host packages programmatically       | Security implications; cross-distro fragmentation; user trust issues    | Not explored in research but implied |

---

## 5. Pattern Confidence Assessment

| Pattern                                             | Confidence      | Key Evidence                                                                             | Key Uncertainty                                                                             |
| --------------------------------------------------- | --------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| CrossHook is an orchestrator                        | **High**        | Source code analysis (04, 06); architecture diagrams (04)                                | None -- structural fact                                                                     |
| Tools must run in host context                      | **High**        | Execution chain (04); tool architecture (01, 02, 04)                                     | Only if CrossHook changes to bundling Wine internally                                       |
| Detect-Guide-Degrade is highest ROI                 | **High**        | Cross-domain analogies (03); existing infrastructure (06); user experience research (08) | Implementation complexity across distro families is unknown                                 |
| Sustainability constrains bundling                  | **Medium-High** | Ecosystem patterns (02, 08); team size reality                                           | Could change if team grows or automation matures                                            |
| VulkanLayer extensions are irrelevant               | **High**        | Architecture analysis (04); contradiction resolution (09)                                | Only if Flatpak adds a mechanism to extend extensions to `flatpak-spawn`-launched processes |
| Proton download is the one viable feature           | **Medium-High** | ProtonUp-Qt precedent (01, 04); filesystem permissions already in manifest               | Write access for Flatpak Steam needs verification                                           |
| Immutable distro argument is weaker than it appears | **Medium**      | Bazzite/SteamOS pre-install tools (05, 08); paradox identified (09)                      | User demographics are unknown (10, Gap #1)                                                  |

---

## 6. Synthesis of Contradictions

`09-contradictions` identifies 11 contradictions/tensions. Here is how the pattern analysis resolves each:

| #   | Contradiction                                           | Resolution                                                                                                                                                                                                                                          | Confidence  |
| --- | ------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| 1.1 | Host delegation: legacy vs. necessity                   | **Necessity wins.** CrossHook's architecture is built around host delegation. Changing this would require rewriting the entire launch chain. The "legacy" framing applies to apps that could theoretically internalize execution; CrossHook cannot. | High        |
| 1.2 | VulkanLayer: solution vs. irrelevant                    | **Irrelevant wins.** Extensions inject into sandbox processes; CrossHook's games run on the host. Empirically testable but architecturally clear.                                                                                                   | High        |
| 1.3 | Immutable distros: bundling argument vs. already solved | **Partially resolved.** Gaming distros pre-install tools (argument weakened). General immutable distros don't (argument holds for this segment). Net: the argument is real but narrower than presented.                                             | Medium      |
| 1.4 | Lutris precedent: delegation vs. bundling               | **Both true; delegation precedent more applicable.** CrossHook doesn't manage Wine installations, so Lutris's runner management is irrelevant. Lutris's Flathub acceptance with `org.freedesktop.Flatpak` IS directly applicable.                   | High        |
| 2.1 | Security vs. function                                   | **Unresolvable tension.** CrossHook requires sandbox escape. Monitor Flathub policy; prepare justification narrative; accept the risk.                                                                                                              | N/A         |
| 2.2 | Partial bundling: best practice vs. worst UX            | **Moot for CrossHook.** Since no tools should be bundled (Tier 1 analysis), the partial bundling UX problem doesn't arise. The tiering in this document is about _mechanisms_ (host delegation, portal, native feature), not bundling levels.       | High        |
| 2.3 | Team capacity vs. maintenance burden                    | **Sustainability wins.** Don't propose what the team can't maintain. The Proton download feature (Tier 3) is the only new maintenance commitment, and it's CrossHook-native code, not an external tool sync.                                        | High        |
| 2.4 | umu-launcher: simplifier vs. dependency                 | **Both true; net positive.** umu reduces CrossHook's dependency surface from many tools to one critical dependency. The "what if umu is missing" question is handled by existing graceful degradation (fallback to direct Proton).                  | Medium-High |
| 2.5 | Flathub: necessary vs. hostile                          | **Navigate, don't avoid.** Flathub is the distribution channel. Lutris precedent shows gaming launchers CAN be accepted. Prepare strong justification for the permission.                                                                           | Medium      |
| 2.6 | GameMode portal: success vs. exception                  | **Acknowledged as exception.** GameMode portal works; no other tool has replicated it. Use the portal for GameMode; don't assume portals will solve other tools.                                                                                    | High        |
| 2.7 | Bottles: closest vs. wrong analogue                     | **Wrong analogue.** Bottles bundles Wine; CrossHook does not. Podman Desktop (thin GUI + host delegation) or VS Code Flatpak (host tool orchestration) are more architecturally relevant.                                                           | High        |

---

## 7. The Decision Framework

The pattern synthesis produces a simple decision tree for any tool:

```
Does the tool need to interact with the game process?
├── YES → Does the game process run on the host?
│   ├── YES → Tool MUST be on the host. Delegate via flatpak-spawn --host.
│   │         CrossHook's role: detect, thread env vars, provide install guidance.
│   └── NO  → N/A (CrossHook always launches on host)
│
└── NO → Can it be implemented as CrossHook-native Rust code?
    ├── YES → Build it as a feature (e.g., Proton download manager)
    └── NO  → Is there a D-Bus portal?
        ├── YES → Use the portal (e.g., GameMode)
        └── NO  → Delegate to host and detect availability
```

Every tool in the research falls cleanly into this tree:

- **Gamescope, MangoHud, winetricks, winecfg**: Game-interactive → host → delegate
- **GameMode**: Not game-interactive (system daemon) → portal exists → use portal
- **Proton download**: Not game-interactive → can be native Rust → build as feature
- **CachyOS opts**: Kernel-level → impossible → detect and display only

---

## Sources

This document is a synthesis of findings from:

- `01-historical.md` -- Historical precedents
- `02-contrarian.md` -- Arguments against bundling
- `03-analogical.md` -- Cross-domain analogies
- `04-systems.md` -- Dependency graphs and permissions
- `05-investigative.md` -- Flathub policies and ecosystem state
- `06-archaeological.md` -- CrossHook's current tool handling
- `07-futurist.md` -- Ecosystem projections
- `08-negative-space.md` -- Blind spots and hidden costs
- `09-contradictions.md` -- Contradiction mapping (Phase 2 crucible)
- `10-evidence.md` -- Evidence quality assessment (Phase 2 crucible)

No additional external research was conducted. All claims trace to the source files above.
