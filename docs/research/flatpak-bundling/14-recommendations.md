# Strategic Recommendations: CrossHook Flatpak Tool Bundling

**Perspective**: Systems Mapper (Capstone Synthesis)
**Date**: 2026-04-15
**Scope**: Final strategic recommendations answering the original research question
**Input files**: All 13 research files (`01-historical.md` through `13-opportunities.md`)

---

## Executive Summary

**Should CrossHook bundle tools in its Flatpak, provide user installation methods, or rely on system paths?**

CrossHook should **not bundle tools inside its Flatpak sandbox**. Instead, it should pursue a **Host-Delegation with Guided Onboarding** strategy: delegate all gaming tool execution to the host via `flatpak-spawn --host` (which it already does), invest heavily in detecting missing tools and guiding users through installation, and build exactly one native feature — a Proton version download manager — as the sole exception. This recommendation is grounded in an architectural fact that 7 of 10 research perspectives independently confirmed: CrossHook is an orchestrator, not an application. Every game it launches runs as a host process via `flatpak-spawn --host`. Every tool that interacts with a game (MangoHud, gamescope, winetricks, winecfg) must therefore also run on the host. Bundling these tools inside the Flatpak sandbox is architecturally circular — they would sit in a sandbox they cannot usefully operate from. The highest-ROI investment is not packaging but onboarding UX: a comprehensive first-run wizard that detects host tools, provides distro-specific install commands, and gracefully degrades when dependencies are missing. CrossHook's existing `platform.rs` abstraction layer and `onboarding/readiness.rs` infrastructure provide the foundation; what's missing is the UI presentation and the breadth of tool coverage.

---

## 1. Recommended Strategy: Host-Delegation with Guided Onboarding

### Strategy Definition

All gaming tools run on the host. CrossHook's Flatpak contains only CrossHook itself (Tauri v2 app, Rust backend, React frontend). The Flatpak manifest declares `--talk-name=org.freedesktop.Flatpak` for `flatpak-spawn --host` access, following the precedent set by Lutris on Flathub. CrossHook detects available host tools, guides installation of missing ones, and degrades gracefully when optional tools are absent.

One exception: CrossHook builds a **native Proton version download manager** in Rust. This is not "bundling" an external tool — it is implementing download/extract functionality that works identically in AppImage and Flatpak, requires no `flatpak-spawn --host`, and uses existing filesystem permissions.

### Why This Strategy (Not the Alternatives)

| Alternative          | Why Rejected                                                                                                                                                                                                                                                                                                                                                        | Key Evidence                                                                                                                         |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **Full Bundle**      | Three structural blockers: gamescope cannot function inside sandbox (needs DRM/KMS), winetricks version must match host Proton (prefix corruption risk), and `flatpak-spawn --host` is still required for game launches regardless. Seven High+ risks, three of which are unmitigable.                                                                              | `12-risks.md` Strategy 1: max risk 25/25 (Critical); `04-systems.md` bundleability matrix; `10-evidence.md` Tier 1 claims #1, #8     |
| **Partial Bundle**   | Worst UX outcome: "uncanny valley" where some tools work and others require installation. Users cannot predict which is which. Heroic Games Launcher issue #4791 documents this exact failure. The most impactful tools (gamescope, MangoHud) are precisely the ones that cannot be bundled for CrossHook's architecture.                                           | `12-risks.md` P-U1 score 20/25 (Critical); `08-negative-space.md` "why not both" trap; `11-patterns.md` Contradiction 2.2 resolution |
| **Hybrid/Extension** | VulkanLayer extensions mount into the sandbox runtime at `/usr/lib/extensions/vulkan/`. CrossHook's games run on the host. Extensions cannot reach host-launched games. The "extension" component of this strategy is architecturally irrelevant. When the extension component is removed, this strategy collapses to Host-Delegation — which is what we recommend. | `10-evidence.md` claim #19 (VulkanLayer misdirection); `09-contradictions.md` Contradiction 1.2; `12-risks.md` E-T1 score 20/25      |

### Justification from All 13 Files

| File                | What It Contributes to This Recommendation                                                                                                                                                                                            |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `01-historical`     | Ecosystem precedents: Lutris, Heroic, Bottles all converge on either internal runner management or host delegation — never sandbox-side bundling of game-interactive tools                                                            |
| `02-contrarian`     | 10 arguments against bundling; architectural mismatch is the strongest (validated by Crucible)                                                                                                                                        |
| `03-analogical`     | Podman Desktop (not Bottles) is CrossHook's closest analogue: thin GUI + host daemon delegation. The Detect-Prompt-Install pattern is universal across macOS, Android, and containerized app platforms                                |
| `04-systems`        | Complete dependency graph proves every gaming tool requires host context. Bundleability matrix: only Proton download is viable                                                                                                        |
| `05-investigative`  | Lutris Flathub precedent validates `--talk-name=org.freedesktop.Flatpak`. Flathub submission is achievable without bundling                                                                                                           |
| `06-archaeological` | CrossHook's `platform.rs` already implements the correct architecture. 13 tools, all routed through `host_command()` / `host_std_command()`. Detection infrastructure exists in `host_command_exists()` and `onboarding/readiness.rs` |
| `07-futurist`       | umu-launcher consolidation (80% probability by 2027) shrinks CrossHook's dependency surface over time, making bundling a wasted investment. Immutable distro growth is real but bounded — gaming distros pre-install tools            |
| `08-negative-space` | First-run experience is "the real battleground." The question is not "what to bundle" but "how to handle missing tools." A great setup wizard beats a mediocre auto-bundle                                                            |
| `09-contradictions` | Resolves the central identity question: CrossHook is an orchestrator, not an application. This determination governs every downstream decision                                                                                        |
| `10-evidence`       | Crucible corrections: VulkanLayer irrelevance (High confidence), Background portal misapplication (High), performance numbers unreliable (High), anti-bundling bias exists but core conclusion holds                                  |
| `11-patterns`       | Five recurring themes converge: orchestrator identity, host-context problem, "already solved" stack, sustainability constraint, Detect-Guide-Degrade pattern. Tool tiering places ALL gaming tools in Tier 1 (delegate to host)       |
| `12-risks`          | Host-Only has 1 High risk (first-run wall, mitigable with onboarding wizard) vs. Full Bundle's 7 High+ risks (3 structural blockers). Risk cartography confirms Host-Delegation as lowest-risk viable strategy                        |
| `13-opportunities`  | Highest-ROI opportunities are all onboarding/UX investments, not bundling. Quick wins (QW-1 through QW-4) are immediately actionable with existing infrastructure                                                                     |

### Crucible Corrections Applied

This recommendation accounts for all four Crucible corrections from `10-evidence.md`:

1. **VulkanLayer extensions are irrelevant** (corrects `03-analogical` Tier 1 and `01-historical` extension recommendations). Games run on the host; extensions inject into sandbox processes. No VulkanLayer extensions should be added to the manifest.

2. **Background portal does not threaten host games** (corrects `05-investigative` Section 4.2). Games launched via `flatpak-spawn --host` are host processes invisible to `org.freedesktop.portal.Background`. However, the Background portal IS relevant to CrossHook's own watchdog process — `RequestBackground` should be called to protect CrossHook itself, not games.

3. **Performance numbers are unreliable** (corrects `08-negative-space` Section 5). The ~50-150ms `flatpak-spawn` overhead and 3-19% seccomp penalty are extrapolated from inapplicable benchmarks. No bundling decisions should be based on these numbers without direct measurement (see Phase 1 benchmark recommendation).

4. **Anti-bundling bias in the corpus** (meta-correction). 6 of 8 Phase 1 files lean anti-bundling; the same evidence (Lutris #6144, gamescope #6, 42% sandbox stat) is cited across 3-4 files, creating false independent corroboration. This recommendation compensates by giving due weight to the immutable distro argument from `07-futurist` while noting that gaming-focused immutable distros pre-install tools, bounding the affected audience to general-purpose immutable distro users.

---

## 2. Per-Tool Recommendation Table

| Tool                      | Decision                               | Mechanism                                                                | Rationale                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Confidence  |
| ------------------------- | -------------------------------------- | ------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------- |
| **Winetricks**            | **Delegate to host**                   | `flatpak-spawn --host winetricks` (existing)                             | Must match the exact Wine/Proton version that owns the game prefix. Bundled winetricks would target sandbox Wine; games run under host Proton. Lutris #6144 documents `libassuan.so.0` symbol errors from this exact mismatch. CrossHook already has detection in `prefix_deps/detection.rs`.                                                                                                                                                                                                                                      | High        |
| **Winecfg**               | **Delegate to host**                   | `flatpak-spawn --host` with Proton's winecfg (existing)                  | IS part of Wine/Proton. Bundling winecfg means bundling Wine (~500MB+). Version mismatch with host Proton causes prefix corruption. Already works via existing host delegation.                                                                                                                                                                                                                                                                                                                                                    | High        |
| **MangoHud**              | **Delegate to host**                   | `MANGOHUD=1` env var threaded via `host_command_with_env()` (existing)   | Vulkan layer that must inject into the game process. Games are host processes. MangoHud bundled in the sandbox would overlay CrossHook's own Tauri window, not the game. VulkanLayer extensions cannot reach host-launched games (Crucible-corrected). Already works via env var threading.                                                                                                                                                                                                                                        | High        |
| **Gamescope**             | **Delegate to host**                   | `flatpak-spawn --host gamescope` as command wrapper (existing)           | Compositor that wraps the game window. Requires DRM master, KMS, and nested Wayland compositor privileges that Flatpak cannot grant. Must wrap a host window (the game), not a sandbox window. `12-risks.md` F-T1 scores 25/25 (Critical). Already works via host delegation.                                                                                                                                                                                                                                                      | High        |
| **GameMode**              | **Use D-Bus portal**                   | `org.freedesktop.portal.GameMode` (version 4)                            | The ONLY tool with proper sandbox-to-host bridging via XDG Desktop Portal. PID namespace translation handled by the portal. Host daemon does the real work (CPU/GPU governor adjustments). CrossHook should use the portal for its own process registration and thread `gamemoderun` for host-launched games. Known PID registration bug exists but affects both portal and direct paths.                                                                                                                                          | High        |
| **umu-launcher**          | **Delegate to host**                   | `flatpak-spawn --host umu-run` with multi-strategy resolution (existing) | Python + Rust + Steam Runtime dependency chain. Pre-built zipapp fails in Flatpak sandbox. Requires host Steam interaction. CrossHook already has robust umu integration: auto/force/skip preference, `resolve_umu_run_path()` with ~20 candidate paths, and `--filesystem=xdg-data/umu:create` in the manifest. As umu matures (80% probability of consolidation by 2027), CrossHook's dependency surface shrinks.                                                                                                                | High        |
| **Proton Manager**        | **Build as native feature**            | Rust GitHub API client downloading GE-Proton/CachyOS-Proton releases     | The sole viable "bundling" — but it's not bundling a tool, it's implementing a feature. Uses existing `--share=network` and `--filesystem=home` permissions. Works identically in AppImage and Flatpak. Does not require `flatpak-spawn --host`. Eliminates the external ProtonUp-Qt dependency. CrossHook's `discover_compat_tools_with_roots()` scanner in `steam/proton.rs` provides the discovery layer; this adds download/extract/version-manage capability. Requires `:rw` access to Flatpak Steam paths (currently `:ro`). | Medium-High |
| **Wine Wayland**          | **Delegate to host** (detect + toggle) | Env var toggle (`WINE_ENABLE_WAYLAND=1`) via `host_command_with_env()`   | Part of Wine. Cannot be separated from the Wine/Proton build. Depends on host Wine/Proton compilation flags. CrossHook's role: detect whether host Wine/Proton supports the Wayland driver, expose a toggle in the profile UI, and thread the env var. No bundling possible or needed.                                                                                                                                                                                                                                             | Medium-High |
| **CachyOS Optimizations** | **Detect and display only**            | `host_command_exists()` for kernel feature probes                        | Kernel-level features (bore/EEVDF schedulers, sysctl tuning, CPU flags, NTSYNC). Impossible to bundle by definition — these are kernel and system-level configurations. CrossHook should detect relevant kernel features (e.g., check for `sched_ext`, NTSYNC availability) and display them in the tool status dashboard to inform the user's system-level tuning.                                                                                                                                                                | High        |

---

## 3. Implementation Phases

Building on the Minimum Viable Flatpak Experience (MVFE) from `11-patterns.md` and the prioritized opportunity matrix from `13-opportunities.md`:

### Phase 1: Foundation (Low Effort, High Impact)

**Goal**: Make the existing Flatpak experience excellent without writing new subsystems. Zero architectural changes.

| Task                                     | What to Do                                                                                                                                                                                                                          | Builds On                                                 | Effort |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------- | ------ |
| **1.1 Expand readiness checks**          | Add detection for gamescope, MangoHud, winetricks/protontricks, GameMode, and Wine Wayland support to `onboarding/readiness.rs`. Currently checks Steam, Proton, umu-run only.                                                      | Existing `host_command_exists()`, `HostDistroFamily` enum | Low    |
| **1.2 Distro-specific install guidance** | For each missing tool, provide the correct install command for the detected distro family. Detect host distro via `flatpak-spawn --host cat /etc/os-release`. Include immutable distro methods (`rpm-ostree`, `brew`, Nix profile). | Existing `build_umu_install_advice()` pattern             | Low    |
| **1.3 Grey out unavailable features**    | Surface missing tool dependencies in the optimization catalog UI as greyed-out entries with "Install X to enable" text and copy-to-clipboard install commands. Replace silent degradation.                                          | Existing `LaunchOptimizationDependencyMissing` validation | Low    |
| **1.4 Verify GameMode portal path**      | Confirm CrossHook uses `org.freedesktop.portal.GameMode` when running as Flatpak, not just `gamemoderun` via `flatpak-spawn --host`. Document the PID registration caveat.                                                          | Existing GameMode integration                             | Low    |
| **1.5 Benchmark `flatpak-spawn --host`** | Measure actual latency: `flatpak-spawn --host /bin/true` vs. native, 100 iterations. Measure CrossHook's real launch chain. Publish results. Closes Evidence Gap #2 from Crucible.                                                  | None                                                      | Low    |
| **1.6 Prepare Flathub justification**    | Document every `flatpak-spawn --host` command CrossHook invokes, why each is necessary, and which portals are used where available (GameMode). Cite Lutris precedent.                                                               | `06-archaeological.md` tool inventory                     | Low    |
| **1.7 Protect `platform.rs` gateway**    | Add architectural decision record (ADR) for the single-abstraction-gateway pattern. Consider CI lint to flag direct `Command::new()` calls that bypass `host_command()` / `host_std_command()`.                                     | Existing architecture                                     | Low    |

### Phase 2: Onboarding Experience (Medium Effort, High Impact)

**Goal**: Build the first-run wizard that transforms the "7-step prerequisite wall" into a guided experience.

| Task                                | What to Do                                                                                                                                                                                                                                                       | Depends On                            | Effort      |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ----------- |
| **2.1 Tool status dashboard**       | Persistent settings panel showing all detected tools with version, path, and status. See `13-opportunities.md` UX-2 for mockup. Makes detection results visible and re-checkable.                                                                                | Phase 1.1 (expanded readiness checks) | Medium      |
| **2.2 Smart platform detection**    | Distinguish SteamOS (all tools pre-installed, skip onboarding), Bazzite/ChimeraOS (mostly installed, confirm), bare Fedora Atomic (full wizard), and traditional distros (standard commands). Resolves the immutable distro paradox from `09-contradictions.md`. | Phase 1.2 (distro detection)          | Medium      |
| **2.3 First-run onboarding wizard** | Step-by-step setup flow: detect environment, present checklist with status, provide install commands per tool per distro, validate after installation, gate game configuration until minimum dependencies met. Re-runnable from settings.                        | Phase 2.1, 2.2                        | Medium-High |
| **2.4 `RequestBackground` portal**  | Call `org.freedesktop.portal.Background.RequestBackground` to prevent xdg-desktop-portal from terminating CrossHook's watchdog process when the Tauri window is minimized during game sessions. Protects CrossHook, not games (per Crucible correction).         | None                                  | Medium      |
| **2.5 Host tool version probing**   | Extend detection to report tool versions (`mangohud --version`, `gamescope --version`, etc.). Warn when host tools are known-incompatible or outdated.                                                                                                           | Phase 1.1                             | Low-Medium  |

### Phase 3: Feature Development (High Effort, High Impact)

**Goal**: Build the Proton version manager and deepen umu-launcher alignment.

| Task                                          | What to Do                                                                                                                                                                                                                                                                          | Depends On                    | Effort |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- | ------ |
| **3.1 Proton version download manager**       | Implement GitHub API client in Rust. Download GE-Proton and CachyOS-Proton releases. Extract to `compatibilitytools.d/`. Version selection, update, and cleanup UI. Integrate with existing `discover_compat_tools_with_roots()`.                                                   | Phase 1.7 (platform.rs ADR)   | High   |
| **3.2 Flatpak Steam write access**            | Negotiate upgrade from `--filesystem=~/.var/app/com.valvesoftware.Steam:ro` to `:rw` for writing compatibility tools into Flatpak Steam's data directory. Requires Flathub justification addendum.                                                                                  | Phase 3.1, Phase 1.6          | Low    |
| **3.3 umu-launcher game database**            | Leverage umu's GAMEID database (umu-protonfixes) to auto-match games to compatibility fixes. Deepens the existing umu integration beyond launch orchestration into per-game tuning.                                                                                                 | Existing umu integration      | Medium |
| **3.4 Protontricks-as-Flatpak investigation** | Test whether CrossHook can invoke the Protontricks Flatpak (`com.github.Matoking.protontricks`) from inside its own Flatpak. If viable, recommend Protontricks Flatpak in the onboarding wizard as a one-click install for prefix management. Closes Evidence Gap #5 from Crucible. | Phase 2.3 (onboarding wizard) | Medium |

---

## 4. Complexity Assessment

| Phase                    | Effort                | Calendar Time                          | Benefit                                                                                                                                                            | Risk                                                                                                                                         | ROI                                                                                                                                                                                       |
| ------------------------ | --------------------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase 1: Foundation**  | ~2-3 developer-weeks  | Can be parallelized; 1-2 weeks elapsed | Eliminates silent failures; provides actionable guidance for every missing tool; establishes Flathub submission readiness; closes critical evidence gaps           | Minimal — extends existing infrastructure with no architectural changes                                                                      | **Very High** — low effort, large UX improvement for Flatpak users, unblocks Flathub submission                                                                                           |
| **Phase 2: Onboarding**  | ~4-6 developer-weeks  | 3-4 weeks elapsed                      | Transforms the 7-step prerequisite wall into a guided experience; resolves the immutable distro paradox via smart detection; protects CrossHook's watchdog process | Low-Medium — new UI components but no backend architecture changes                                                                           | **High** — addresses the single biggest UX gap (first-run wall, scored 16/25 in risk cartography); the onboarding wizard is the most impactful investment regardless of bundling strategy |
| **Phase 3: Feature Dev** | ~6-10 developer-weeks | 4-8 weeks elapsed                      | Eliminates ProtonUp-Qt dependency; provides in-app Proton management; deepens umu alignment; potentially eliminates a host dependency (Protontricks)               | Medium — new subsystem (Proton manager) with ongoing maintenance commitment for GitHub API integration, archive handling, version management | **Medium-High** — genuine new capability that differentiates CrossHook; maintenance cost is bounded (CrossHook-native code, not external tool sync)                                       |

### Phase Dependencies

```
Phase 1 (Foundation)
  ├── 1.1 Readiness checks ──► 2.1 Tool dashboard ──► 2.3 Onboarding wizard
  ├── 1.2 Distro detection ──► 2.2 Platform detection ──► 2.3 Onboarding wizard
  ├── 1.6 Flathub justification ──► 3.2 Steam write access
  └── 1.7 platform.rs ADR ──► 3.1 Proton manager

Phase 2 (Onboarding)
  └── 2.3 Onboarding wizard ──► 3.4 Protontricks investigation

Phase 3 (Feature Dev)
  └── Independent of Phase 2 except for 3.4
```

Phases 1 and 2 are sequential. Phase 3 tasks (except 3.4) can begin in parallel with Phase 2 if developer bandwidth allows.

---

## 5. Decision Criteria: When to Revisit This Strategy

This recommendation is grounded in CrossHook's current architecture and the Flatpak ecosystem as of April 2026. The following signals should trigger a strategy review:

### Signals That Would Strengthen Host-Delegation

| Signal                                                                      | What It Means                                                                                                                                                    | Likelihood (2 years)                             |
| --------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ |
| **umu-launcher reaches ubiquity**                                           | CrossHook's host dependency surface shrinks to umu-run + Steam. The "7-step prerequisite" becomes a "2-step prerequisite." Bundling argument collapses entirely. | High (80% per `07-futurist`)                     |
| **Flatpak implements fine-grained `flatpak-spawn` filtering** (issue #5538) | CrossHook can declare exactly which host commands it needs, dramatically reducing its permission footprint and easing Flathub review.                            | Medium (30-50%)                                  |
| **XDG Desktop Portal adds gaming portals**                                  | More tools gain portal paths (like GameMode), reducing the need for `flatpak-spawn --host`. Each new portal is a tool CrossHook no longer needs to delegate.     | Low-Medium (20-40%)                              |
| **User demographics show gaming-distro dominance**                          | If 80%+ of CrossHook Flatpak users are on Bazzite/SteamOS/ChimeraOS where tools are pre-installed, the onboarding wizard handles the majority case trivially.    | Unknown — measure via opt-in analytics or survey |

### Signals That Would Weaken Host-Delegation (Trigger Re-evaluation)

| Signal                                                                 | What It Means                                                                                                                                                                                                | Likelihood (2 years)                                                                                              | Threshold for Action                                                                                                            |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| **Flathub rejects CrossHook's `org.freedesktop.Flatpak` permission**   | Distribution channel blocked. Must find alternatives (self-hosted repo, AppImage-only, or reduced-permission architecture).                                                                                  | Low (15-25%, given Lutris precedent)                                                                              | Immediate re-evaluation; consider Flatpak-without-host-access mode that disables game launching                                 |
| **User demographics show bare-immutable-distro dominance**             | If 50%+ of CrossHook Flatpak users are on bare Fedora Atomic or NixOS without pre-installed gaming tools, the onboarding wizard alone may not suffice.                                                       | Low (unlikely given the correlation between "wanting a game trainer launcher" and "having a gaming-ready system") | Consider selective bundling of the one viable tool: Proton download (already planned in Phase 3)                                |
| **A new sandbox mechanism enables tool injection into host processes** | If Flatpak or a new portal gains the ability to extend sandbox-installed tools (e.g., Vulkan layers) to `flatpak-spawn --host`-launched processes, some bundling becomes viable.                             | Very Low (<10%)                                                                                                   | Evaluate per-tool: MangoHud and gamescope would be candidates; winetricks/winecfg version-coupling remains a blocker regardless |
| **CrossHook internalizes Wine/Proton execution** (architectural shift) | If CrossHook stops using `flatpak-spawn --host` and instead bundles Wine/Proton to run games inside the sandbox, the entire tool bundling calculus changes. This would be a fundamental architectural pivot. | Very Low (<5%, would require rewriting the entire launch chain)                                                   | Full strategy reassessment — but this is a project identity question, not a Flatpak packaging question                          |

### Monitoring Cadence

- **Quarterly**: Check umu-launcher status, Flatpak release notes for portal/permission changes, Flathub policy updates
- **Per-release**: Review user feedback for Flatpak-specific pain points; track onboarding wizard completion rates if implemented
- **Annually**: Full strategy review against these criteria

---

## 6. What NOT to Do

Drawing on `12-risks.md` risk cartography, `09-contradictions.md` contradiction resolutions, and `10-evidence.md` Crucible corrections:

### Anti-Pattern 1: Do Not Bundle Gaming Tools Inside the Flatpak Sandbox

**Why**: Every gaming tool (MangoHud, gamescope, winetricks, winecfg) must interact with the game process, which is a HOST process launched via `flatpak-spawn --host`. A tool inside the sandbox cannot inject Vulkan layers into a host process, cannot wrap a host window in a compositor, and cannot manipulate a Wine prefix owned by a host Proton installation. Bundling creates the illusion of capability without the reality.

**Evidence**: `12-risks.md` F-T1 (25/25), F-T2 (20/25), F-T3 (20/25) — three Critical structural blockers. `04-systems.md` bundleability matrix. `11-patterns.md` Theme 2 (Host-Context Problem, 6/10 files).

### Anti-Pattern 2: Do Not Add VulkanLayer Extensions to the Manifest

**Why**: VulkanLayer extensions mount into the sandbox runtime at `/usr/lib/extensions/vulkan/`. CrossHook's games do not run in the sandbox. The Vulkan layer search path (`VK_LAYER_PATH`) is per-process; a layer in the sandbox's path is not on the host game's search path. Adding VulkanLayer extensions would apply MangoHud to CrossHook's own Tauri/WebKitGTK window rendering — functionally useless and potentially confusing.

**Evidence**: `10-evidence.md` claim #19 (Crucible correction, High confidence). `09-contradictions.md` Contradiction 1.2 (resolved: irrelevant wins). `12-risks.md` F-T4 (15/25), P-T1 (15/25).

### Anti-Pattern 3: Do Not Implement Dual-Path "Host + Bundled Fallback" Logic

**Why**: Maintaining two execution paths per tool (sandbox-bundled vs. host-delegated) doubles the testing surface. The "which version is running?" ambiguity creates the worst user experience of any strategy (`12-risks.md` P-U1 scores 20/25, the highest UX risk across all strategies). Heroic Games Launcher issues #4791, #4588, and #4729 document the support burden this pattern creates. Lapce's Flatpak fallback experience further confirms the pattern fails in practice.

**Evidence**: `08-negative-space.md` "why not both" trap. `12-risks.md` Strategy 2 assessment. `11-patterns.md` Contradiction 2.2 resolution (moot for CrossHook since no tools should be bundled).

### Anti-Pattern 4: Do Not Engineer Around the Background Portal for Game Protection

**Why**: The Crucible identified this concern as likely misapplied. Games launched via `flatpak-spawn --host` are HOST processes. `org.freedesktop.portal.Background` monitors SANDBOX processes. Game PIDs live in the host PID namespace, invisible to the portal. Engineering defensive measures for a non-existent threat wastes effort. (Do call `RequestBackground` to protect CrossHook's OWN watchdog process — that IS a sandbox process at risk.)

**Evidence**: `10-evidence.md` Assumption 4 (Crucible correction, High confidence). `12-risks.md` H-T4 (corrected to 1/25).

### Anti-Pattern 5: Do Not Use Unreliable Performance Numbers to Justify Decisions

**Why**: The ~50-150ms `flatpak-spawn` latency estimate from `08-negative-space.md` is extrapolated from Flatpak cold-start benchmarks that measure a completely different operation. The 3-19% seccomp overhead measures games running INSIDE sandboxes, but CrossHook's games run on the HOST. Neither number has been measured for CrossHook's actual execution path. Making engineering investments based on unmeasured overhead is premature optimization. Benchmark first (Phase 1.5), then decide.

**Evidence**: `10-evidence.md` Theme G (performance numbers unreliable, High confidence). `12-risks.md` F-T6 (corrected to 4/25).

### Anti-Pattern 6: Do Not Compete with Bottles or Lutris on Tool Bundling

**Why**: CrossHook's value proposition is trainer orchestration, not game launcher replacement. Bottles bundles Wine because it IS a Wine prefix manager — Wine is its core. Lutris manages Wine runners because runner management is its core. CrossHook launches trainers alongside games; Wine/Proton is a means, not the end. Pursuing bundling pulls CrossHook toward an identity (game launcher) where established projects have years of head start and deeper integration, while abandoning the orchestrator identity where CrossHook is differentiated.

**Evidence**: `09-contradictions.md` Contradiction 1.1 (orchestrator identity, High confidence). `12-risks.md` F-S1 (architectural identity crisis). `11-patterns.md` Theme 1 (Orchestrator Identity, 7/10 files). `10-evidence.md` Assumption 2 (Podman Desktop, not Bottles, is the correct analogue).

### Anti-Pattern 7: Do Not Assume Immutable Distro Users Cannot Install Tools

**Why**: The immutable distro bundling argument from `07-futurist` is real but bounded. Gaming-focused immutable distros (Bazzite, SteamOS, ChimeraOS) — the ones most likely to have CrossHook users — pre-install ALL gaming tools. General-purpose immutable distros (Fedora Atomic, NixOS) lack pre-installed tools but offer installation methods (`rpm-ostree install`, Nix profiles, toolbox). The "users can't install tools" framing conflates "harder to install" with "impossible to install." Smart platform detection (Phase 2.2) resolves this by providing the right guidance for the right platform.

**Evidence**: `09-contradictions.md` Contradiction 1.3 (partially resolved, Medium confidence). `10-evidence.md` Gap #1 (user base composition unknown). `11-patterns.md` Connection 5 (Bazzite/SteamOS pre-installation paradox).

---

## 7. Summary: The Answer

**Question**: Should CrossHook bundle tools in its Flatpak, provide user installation methods, or rely on system paths?

**Answer**: Provide user installation methods (option 2), reinforced by relying on system paths (option 3). Do not bundle (option 1).

Concretely:

- **Rely on system paths** for ALL gaming tools (gamescope, MangoHud, winetricks, winecfg, umu-launcher, Wine Wayland). CrossHook already does this correctly via `platform.rs`.
- **Provide user installation methods** through a comprehensive onboarding wizard that detects missing tools, identifies the user's platform, and provides distro-specific install commands. This is the highest-ROI investment in the entire research scope.
- **Use the D-Bus portal** for GameMode — the one tool where the ecosystem provides a proper sandbox-to-host bridge.
- **Build one native feature** — a Proton version download manager — as the sole exception to the "do not bundle" rule, because it implements new functionality rather than duplicating a host tool.
- **Detect and display** kernel-level features (CachyOS optimizations) as informational, with no pretense of control.

This strategy has the lowest risk profile (1 High risk, mitigable), the lowest maintenance cost, the cleanest Flathub submission story, and the most future-proof alignment with umu-launcher consolidation. It matches CrossHook's architectural identity as an orchestrator and follows the proven Podman Desktop pattern of thin GUI + host delegation + guided onboarding.

---

## Sources

This document synthesizes findings from all 13 research files:

- `01-historical.md` — Flatpak bundling precedents
- `02-contrarian.md` — Arguments against bundling
- `03-analogical.md` — Cross-domain analogies
- `04-systems.md` — Dependency graphs and permission models
- `05-investigative.md` — Current Flatpak gaming ecosystem
- `06-archaeological.md` — CrossHook's tool detection architecture
- `07-futurist.md` — Ecosystem trajectory projections
- `08-negative-space.md` — Blind spots and hidden costs
- `09-contradictions.md` — Contradiction mapping (Phase 2 crucible)
- `10-evidence.md` — Evidence quality assessment (Phase 2 crucible)
- `11-patterns.md` — Cross-cutting pattern synthesis (Phase 3)
- `12-risks.md` — Risk cartography (Phase 3)
- `13-opportunities.md` — Opportunities and quick wins (Phase 3)

No additional external research was conducted for this capstone. All claims trace to the source files above.

## CrossHook Follow-up

- Issue `#270` is now implemented through the host tool dashboard + shared capability-gating work tracked in `docs/internal/host-tool-dashboard.md`.
- The shipped implementation covers the recommended detection/guidance path:
  - shared host readiness snapshot consumption
  - one Settings-hosted dashboard surface
  - onboarding handoff into the dashboard
  - panel-level capability gating for host-tool-dependent workflows
