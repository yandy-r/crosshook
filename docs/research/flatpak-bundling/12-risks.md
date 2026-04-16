# Risk Cartography: Flatpak Tool Bundling Strategies

**Perspective**: Investigative Journalist (Synthesis Phase)
**Date**: 2026-04-15
**Scope**: Map technical, maintenance, UX, distribution, and strategic risks for each bundling strategy, scored by likelihood x impact
**Input files**: `01-historical.md` through `10-evidence.md`

---

## Executive Summary

This document maps risks across four bundling strategies for CrossHook's Flatpak package: **Full Bundle**, **Partial Bundle**, **Host-Only**, and **Hybrid/Extension-Based**. Each risk is scored by likelihood (1-5) x impact (1-5), yielding a risk score (1-25). Three Crucible corrections from `10-evidence.md` are applied throughout:

1. **Performance numbers are unreliable** — the ~50-150ms `flatpak-spawn` overhead and 3-19% seccomp penalty are extrapolated from inapplicable benchmarks; neither has been measured for CrossHook's actual execution path.
2. **Background portal does not apply to host games** — `org.freedesktop.portal.Background` monitors sandbox processes, not host processes spawned via `flatpak-spawn --host`. Games are host processes.
3. **Anti-bundling bias exists** — 6 of 8 Phase 1 files argue against bundling; the same evidence (Lutris #6144, gamescope #6, 42% sandbox stat) is cited across 3-4 files each, inflating apparent corroboration. This assessment compensates by weighting pro-bundling and usability arguments more heavily than the raw citation count suggests.

**Key finding**: No strategy is risk-free. Host-Only carries the lowest technical and maintenance risk but the highest UX risk on immutable distros. Full Bundle carries the highest maintenance and distribution risk but addresses the first-run experience gap. The Hybrid/Extension-Based approach offers the best risk profile overall, but its viability depends on unresolved questions about VulkanLayer extension behavior with host-launched games.

---

## Methodology

### Risk Scoring Framework

| Dimension      | Scale             | Definition                                        |
| -------------- | ----------------- | ------------------------------------------------- |
| **Likelihood** | 1 = Very Unlikely | < 10% probability in the next 2 years             |
|                | 2 = Unlikely      | 10-30% probability                                |
|                | 3 = Possible      | 30-60% probability                                |
|                | 4 = Likely        | 60-85% probability                                |
|                | 5 = Near Certain  | > 85% probability                                 |
| **Impact**     | 1 = Negligible    | Minor inconvenience; workaround exists            |
|                | 2 = Minor         | Affects some users; degraded experience           |
|                | 3 = Moderate      | Significant user-facing issue; support burden     |
|                | 4 = Major         | Core functionality broken for a user segment      |
|                | 5 = Critical      | Project viability threatened or Flathub rejection |

**Risk Score** = Likelihood x Impact

| Score Range | Rating       | Meaning                                     |
| ----------- | ------------ | ------------------------------------------- |
| 1-6         | **Low**      | Acceptable; monitor                         |
| 7-12        | **Medium**   | Plan mitigation; not urgent                 |
| 13-18       | **High**     | Requires active mitigation before shipping  |
| 19-25       | **Critical** | Strategy may be unviable without resolution |

### Evidence Grading

Each risk references the research file(s) supporting it and notes where Crucible corrections modify the assessment. Risks are tagged:

- **[CORRECTED]** — Risk score adjusted due to Crucible findings
- **[BIAS-COMPENSATED]** — Risk may be overstated by anti-bundling bias in the corpus; score adjusted upward for pro-bundling positions or downward for anti-bundling positions

---

## Strategy 1: Full Bundle

> Bundle all major tools (Wine/Proton, gamescope, MangoHud, GameMode, winetricks, umu-launcher) inside CrossHook's Flatpak sandbox.

### 1.1 Technical Risks

| #    | Risk                                                                                                                                                                                                              | L   | I   | Score  | Rating   | Evidence                          | Notes                                                                                                                                                                                                                                                                                                                                      |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ------ | -------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| F-T1 | **Gamescope cannot function inside the sandbox** — requires DRM master, KMS, and compositor privileges that Flatpak cannot grant                                                                                  | 5   | 5   | **25** | Critical | §02, §04, §05                     | Gamescope must create a nested Wayland compositor with direct DRM access. Flatpak's `--device=dri` is insufficient for KMS operations. The VulkanLayer extension for gamescope is acknowledged as a "hack" by maintainers (§01, §05). Even if bundled, gamescope cannot attach to host-launched game processes.                            |
| F-T2 | **Winetricks/winecfg version mismatch with host Proton** — bundled winetricks expects a specific Wine version; host Proton may be different                                                                       | 5   | 4   | **20** | Critical | §01 (Lutris #6144), §02, §04      | Wine prefix architecture requires winetricks to match the Wine version that owns the prefix. Bundled winetricks would target CrossHook's sandbox Wine, but games run under HOST Proton. The Lutris Flatpak #6144 demonstrates this exact failure: `libassuan.so.0` symbol errors when sandbox winetricks conflicts with runtime libraries. |
| F-T3 | **`flatpak-spawn --host` still required for games** — even with all tools bundled, games must launch on the host because Proton/Wine needs host GPU drivers, kernel features, and Steam integration               | 5   | 4   | **20** | Critical | §04, §06                          | CrossHook's core function is launching games via `flatpak-spawn --host`. Bundling tools does not eliminate this dependency. The `--talk-name=org.freedesktop.Flatpak` permission remains mandatory. Bundled tools would sit inside the sandbox while the actual execution still happens on the host.                                       |
| F-T4 | **VulkanLayer extensions do not reach host-launched games** — MangoHud bundled as a VulkanLayer extension would apply to CrossHook's own Tauri/WebKitGTK process, not to games spawned via `flatpak-spawn --host` | 5   | 3   | **15** | High     | §04, §09 (Contradiction 1.2), §10 | Vulkan layer discovery is per-process via `VK_LAYER_PATH`. A layer in the sandbox's `/usr/lib/extensions/vulkan/` is not on the host game's Vulkan search path. This is an architecturally fundamental mismatch.                                                                                                                           |
| F-T5 | **Binary path forking** — dual code paths needed (bundled vs. host) for every tool invocation, doubling complexity                                                                                                | 4   | 3   | **12** | Medium   | §02, §06                          | `platform.rs` currently has a clean abstraction: all tools go through `host_command()`. Bundling introduces per-tool routing: "is the bundled version appropriate, or should we use the host version?" This requires version detection, comparison, and fallback logic for each tool.                                                      |
| F-T6 | **[CORRECTED] Performance overhead from sandbox tool execution** — tools running inside the sandbox may have latency penalties                                                                                    | 2   | 2   | **4**  | Low      | §08, §10 (Correction)             | The 50-150ms latency and 3-19% seccomp claims are extrapolated from inapplicable benchmarks. Actual overhead is unmeasured. For tools like winetricks (interactive, infrequent) overhead is negligible even if real. For MangoHud (continuous overlay), the tool must run on the host anyway.                                              |

### 1.2 Maintenance Risks

| #    | Risk                                                                                                                                         | L   | I   | Score  | Rating   | Evidence                     | Notes                                                                                                                                                                                                                                                                      |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ------ | -------- | ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F-M1 | **240+ test scenario explosion** — 6 tools x 8 host configurations x 5 distros creates a test matrix small teams cannot sustain              | 5   | 4   | **20** | Critical | §08 §3                       | Mathematical certainty. Each bundled tool introduces: (a) bundled version works / doesn't work, (b) host version present / absent, (c) versions match / conflict. Flathub docs explicitly warn about maintenance burden scaling with bundled modules.                      |
| F-M2 | **Security patch lag** — CrossHook must track and ship patches for 6+ independent upstream projects                                          | 4   | 4   | **16** | High     | §02, §08 §5                  | CVE response time for bundled tools depends on CrossHook's release cadence, not distro maintainers. A critical MangoHud or Wine vulnerability requires CrossHook to rebuild and ship within days. Post-CVE-2026-34078, Flathub scrutiny on security posture is heightened. |
| F-M3 | **Upstream breakage propagation** — changes in bundled tool APIs or behaviors break CrossHook                                                | 4   | 3   | **12** | Medium   | §01, §08                     | Lutris's experience with bundled Wine runners shows that upstream Wine changes (deprecations, behavior changes) require launcher-side patches. CrossHook would inherit this burden for every bundled tool.                                                                 |
| F-M4 | **[BIAS-COMPENSATED] Maintenance cliff — bundled tools become liability** — as team bandwidth fluctuates, bundled tools fall behind upstream | 3   | 4   | **12** | Medium   | §02, §08 §5, §10 (Bias note) | The "maintenance cliff" argument appears in both §02 and §08, but §10 notes it assumes all 7 tools bundled — a scenario no one proposes. Adjusted downward because the actual proposal would be 0-2 tools. Still a real risk for any bundled component.                    |

### 1.3 UX Risks

| #    | Risk                                                                                                                                                | L   | I   | Score       | Rating | Evidence    | Notes                                                                                                                                                                                                                                                                                                                                                                   |
| ---- | --------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ----------- | ------ | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F-U1 | **[BIAS-COMPENSATED] "Just works" first-run on immutable distros** — bundling eliminates the 7-step prerequisite installation for bare-system users | —   | —   | **Benefit** | —      | §07, §08 §1 | This is NOT a risk — it's the primary argument FOR full bundling. Included for completeness. On bare Fedora Atomic, bundling would eliminate the need for users to install 7 host tools. However, per the Crucible correction, gaming-focused immutable distros (Bazzite, SteamOS) pre-install these tools, limiting the audience to general-purpose immutable distros. |
| F-U2 | **Stale bundled tools degrade experience** — users with newer host tools can't benefit because CrossHook uses older bundled versions                | 4   | 3   | **12**      | Medium | §08 §5      | Power users who maintain current host tools via package managers would get worse performance or features from stale bundled versions. Requires version comparison + preference logic to avoid.                                                                                                                                                                          |
| F-U3 | **Download size explosion** — 800MB-1.5GB estimated full bundle vs. ~50MB current AppImage                                                          | 4   | 2   | **8**       | Medium | §02         | Individual sizes verified via `pacman -Si`. Full transitive dependency total is estimated, not measured. Flatpak OSTree dedup reduces actual disk impact for users who already have the GNOME runtime. Still, 800MB+ initial download is a barrier on slow connections.                                                                                                 |

### 1.4 Distribution Risks

| #    | Risk                                                                                                                                                     | L   | I   | Score  | Rating | Evidence                    | Notes                                                                                                                                                                                                                                                                                                                             |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ------ | ------ | --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F-D1 | **Flathub rejection or extended review** — the combination of broad sandbox permissions + bundled tools may trigger policy concerns                      | 3   | 5   | **15** | High   | §05, §09 (Tension 2.5), §10 | Lutris is accepted with `--talk-name=org.freedesktop.Flatpak`, setting precedent. But Lutris is a well-established project with years of Flathub history. A new app requesting broad permissions AND bundling large tool stacks faces more scrutiny. Post-CVE-2026-34078, reviewer skepticism toward sandbox escapes is elevated. |
| F-D2 | **LGPL compliance burden** — Wine/winetricks LGPL requires source availability for modifications; per-module license installation scales with tool count | 3   | 2   | **6**  | Low    | §08 §4                      | Most tools (MangoHud MIT, GameMode BSD-3, gamescope BSD-2) are permissively licensed. Wine/winetricks LGPL requires source distribution for modifications. Flathub mandates per-module license files. Administrative burden, not a blocker.                                                                                       |

### 1.5 Strategic Risks

| #    | Risk                                                                                                                                                                                 | L   | I   | Score  | Rating | Evidence                | Notes                                                                                                                                                                                                                  |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --- | --- | ------ | ------ | ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F-S1 | **Architectural identity crisis** — bundling makes CrossHook a "launcher" rather than an "orchestrator," competing with Lutris/Bottles                                               | 3   | 4   | **12** | Medium | §09 (Contradiction 1.1) | CrossHook's value proposition is trainer orchestration, not game launcher replacement. Bundling tools pulls it toward the launcher identity, where Lutris and Bottles have years of head start and deeper integration. |
| F-S2 | **Ecosystem evolution makes bundling obsolete** — umu-launcher consolidation (80% probability by 2027), systemd-appd, or improved portals could eliminate the need for bundled tools | 3   | 3   | **9**  | Medium | §07                     | Sunk engineering cost if the ecosystem solves the problem. umu-launcher is actively consolidating the Proton launch chain. If umu becomes ubiquitous, bundling Proton/Wine is wasted effort.                           |

### Strategy 1 Summary

| Dimension             | Top Risk Score | Count High+ |
| --------------------- | -------------- | ----------- |
| Technical             | 25 (Critical)  | 4           |
| Maintenance           | 20 (Critical)  | 2           |
| UX                    | 12 (Medium)    | 0           |
| Distribution          | 15 (High)      | 1           |
| Strategic             | 12 (Medium)    | 0           |
| **Total High+ Risks** |                | **7**       |

**Assessment**: Full bundling is the highest-risk strategy. Three Critical risks (gamescope impossibility, winetricks version mismatch, host execution still required) are structural and cannot be mitigated by engineering effort. The testing matrix and security patch obligations compound the maintenance burden. The UX benefit (first-run simplicity) is real but limited to non-gaming immutable distros.

---

## Strategy 2: Partial Bundle

> Bundle a subset of tools (e.g., MangoHud, GameMode) while requiring others (gamescope, Proton, winetricks) on the host.

### 2.1 Technical Risks

| #    | Risk                                                                                                                                             | L   | I   | Score  | Rating | Evidence           | Notes                                                                                                                                                                                                                              |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------ | --- | --- | ------ | ------ | ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P-T1 | **VulkanLayer MangoHud does not reach host games** — same as F-T4; MangoHud bundled via VulkanLayer extension applies to sandbox processes only  | 5   | 3   | **15** | High   | §04, §09, §10      | Host-launched games via `flatpak-spawn --host` do not inherit the sandbox's Vulkan layer path. MangoHud must run where the game runs — on the host.                                                                                |
| P-T2 | **GameMode already works via portal** — bundling GameMode is redundant; `org.freedesktop.portal.GameMode` (v4) provides sandbox-to-host bridging | 4   | 1   | **4**  | Low    | §01, §02, §04, §05 | GameMode is the ONLY tool with proper portal support. Bundling it adds no value over the portal path. The portal has a PID registration bug (§04) but that affects both bundled and portal paths.                                  |
| P-T3 | **Dual execution model complexity** — some tools run in sandbox, others on host; code paths diverge                                              | 4   | 3   | **12** | Medium | §02, §06           | `platform.rs` would need per-tool routing: "is this a bundled tool? Use sandbox path. Is this a host tool? Use `flatpak-spawn --host`." Each decision point is a potential bug.                                                    |
| P-T4 | **Version coupling between bundled and host tools** — bundled MangoHud may expect a Vulkan ICD version different from the host's                 | 3   | 3   | **9**  | Medium | §04, §08           | MangoHud must link against the same Vulkan ICD as the game. If MangoHud is in the sandbox but the game is on the host, the Vulkan loader contexts differ. This is a variant of the "VulkanLayer doesn't reach host games" problem. |

### 2.2 Maintenance Risks

| #    | Risk                                                                                                                     | L   | I   | Score  | Rating | Evidence | Notes                                                                                                                                                                      |
| ---- | ------------------------------------------------------------------------------------------------------------------------ | --- | --- | ------ | ------ | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P-M1 | **Reduced but non-trivial test matrix** — each bundled tool still introduces a host-configuration interaction surface    | 4   | 3   | **12** | Medium | §08 §3   | Even 2 bundled tools x 8 host configs x 5 distros = 80 scenarios. Better than 240+ but still significant for a small team.                                                 |
| P-M2 | **Asymmetric update responsibility** — some tools are CrossHook's responsibility (bundled), others are the user's (host) | 4   | 3   | **12** | Medium | §08 §5   | When a bundled tool has a vulnerability, CrossHook must patch. When a host tool has a vulnerability, the user/distro must patch. Different timelines create version drift. |

### 2.3 UX Risks

| #    | Risk                                                                                                                                                                       | L   | I   | Score               | Rating   | Evidence    | Notes                                                                                                                                                                                                                                                                                                                                                                         |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ------------------- | -------- | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P-U1 | **Uncanny valley — inconsistent "what works" mental model** — some tools work out-of-the-box, others require manual host installation; users cannot predict which is which | 5   | 4   | **20**              | Critical | §08 §2      | Heroic Games Launcher demonstrates this exact problem (issue #4791): MangoHud/gamescope require Flatpak VulkanLayer extensions but error messages don't explain this. Users report "gamescope is broken" when it simply isn't installed. Heroic required dedicated PRs (#4588, #4729) for Flatpak-aware error messages. The support burden from confused users is documented. |
| P-U2 | **Blame misattribution** — users blame CrossHook for host tool failures because some tools "just work"                                                                     | 4   | 3   | **12**              | Medium   | §08 §2      | When MangoHud works (bundled) but gamescope doesn't (host), users file bugs against CrossHook. Every "it doesn't work" ticket requires triage: is the issue in CrossHook, the bundled tool, the host tool, or the interaction?                                                                                                                                                |
| P-U3 | **[BIAS-COMPENSATED] Partial first-run improvement** — partially reduces the 7-step prerequisite but doesn't eliminate it                                                  | —   | —   | **Partial benefit** | —        | §07, §08 §1 | Reduces host prerequisites from ~7 to ~4-5 steps. Better than Host-Only but doesn't achieve the "just works" goal. The incomplete improvement may frustrate more than a clear "install these tools" instruction.                                                                                                                                                              |

### 2.4 Distribution Risks

| #    | Risk                                                                                                                                                     | L   | I   | Score  | Rating | Evidence | Notes                                                                                                                                                                                                                                    |
| ---- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ------ | ------ | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P-D1 | **Same Flathub permission requirements** — still needs `--talk-name=org.freedesktop.Flatpak` for host tools; bundled tools don't reduce permission scope | 4   | 3   | **12** | Medium | §05, §09 | Partial bundling does not improve the Flathub review story. CrossHook still requires the broadest sandbox escape permission for host game launches. The bundled tools add manifest complexity without reducing the permission footprint. |
| P-D2 | **Increased package size without full benefit** — each bundled tool adds size but doesn't eliminate host dependencies                                    | 3   | 2   | **6**  | Low    | §02      | Moderate size increase (50-200MB over base) for partial benefit. Not a major concern but contributes to the "worst of both worlds" perception.                                                                                           |

### 2.5 Strategic Risks

| #    | Risk                                                                                                                                                          | L   | I   | Score  | Rating | Evidence | Notes                                                                                                                                                                          |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ------ | ------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| P-S1 | **"Why not both" trap** — partial bundling feels like a reasonable compromise but satisfies neither the "it just works" crowd nor the "keep it minimal" crowd | 4   | 3   | **12** | Medium | §08 §8   | §08 explicitly identifies this as the "why not both" trap. The strategy attracts criticism from both directions: too much for minimalists, too little for convenience seekers. |
| P-S2 | **Ecosystem renders bundled tools obsolete** — if GameMode portal improves or umu-launcher handles MangoHud integration, the bundled tools become dead weight | 3   | 2   | **6**  | Low    | §07      | Lower sunk cost than full bundling. If 1-2 bundled tools become unnecessary, removing them is feasible.                                                                        |

### Strategy 2 Summary

| Dimension             | Top Risk Score | Count High+ |
| --------------------- | -------------- | ----------- |
| Technical             | 15 (High)      | 1           |
| Maintenance           | 12 (Medium)    | 0           |
| UX                    | 20 (Critical)  | 1           |
| Distribution          | 12 (Medium)    | 0           |
| Strategic             | 12 (Medium)    | 0           |
| **Total High+ Risks** |                | **2**       |

**Assessment**: Partial bundling has fewer structural impossibilities than full bundling but introduces the most dangerous UX risk: the uncanny valley of inconsistent behavior. The Heroic Games Launcher experience provides direct evidence that partial bundling generates a high support burden from confused users. The "worst of both worlds" strategic positioning is the defining weakness.

---

## Strategy 3: Host-Only (Current Model)

> Bundle nothing. All tools run on the host via `flatpak-spawn --host`. CrossHook detects, guides, and gracefully degrades.

### 3.1 Technical Risks

| #    | Risk                                                                                                             | L   | I   | Score  | Rating | Evidence                 | Notes                                                                                                                                                                                                                                                                                                                                                                |
| ---- | ---------------------------------------------------------------------------------------------------------------- | --- | --- | ------ | ------ | ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H-T1 | **[CORRECTED] `flatpak-spawn --host` latency** — every tool invocation routes through D-Bus, adding overhead     | 3   | 1   | **3**  | Low    | §08 §5, §10 (Correction) | The 50-150ms estimate is extrapolated from inapplicable Flatpak startup benchmarks. Actual `flatpak-spawn --host` overhead is unmeasured. For game launches (seconds-scale startup), even 150ms would be negligible. For rapid-fire tool probes during detection, caching (already implemented via `OnceLock` in `runtime_helpers.rs`) eliminates repeated overhead. |
| H-T2 | **`flatpak-spawn` silently drops `.env()` calls** — environment variables must be threaded via `--env=K=V` args  | 2   | 2   | **4**  | Low    | §04, §06                 | Already solved. `platform.rs` correctly uses `--env=K=V` for all env vars and a `0600` env file for sensitive values. This is a known constraint, not an open risk.                                                                                                                                                                                                  |
| H-T3 | **Host tool version fragmentation** — users may have ancient, incompatible, or misconfigured host tools          | 4   | 3   | **12** | Medium | §01, §06                 | Detection is implemented (`host_command_exists()`, onboarding readiness checks). But CrossHook cannot control host tool versions. A user with MangoHud 0.6.x vs. 0.7.x may get different behavior. No version negotiation exists today.                                                                                                                              |
| H-T4 | **[CORRECTED] Background portal does not threaten host games** — games are host processes, not sandbox processes | 1   | 1   | **1**  | Low    | §05, §10 (Correction)    | My original investigative report (§05 Section 4.2) warned about `org.freedesktop.portal.Background` killing games. The Crucible correctly identified this as likely wrong: games launch via `flatpak-spawn --host`, creating host PID namespace processes. The Background portal monitors sandbox processes. This is NOT a risk for CrossHook's architecture.        |

### 3.2 Maintenance Risks

| #    | Risk                                                                                                                                             | L   | I   | Score | Rating | Evidence | Notes                                                                                                                                                                 |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------ | --- | --- | ----- | ------ | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H-M1 | **Minimal maintenance burden** — no bundled tools means no upstream tracking, no security patches, no compatibility testing                      | 5   | 0   | **0** | None   | §02      | This is a benefit, not a risk. The zero-bundle approach has the lowest maintenance cost by definition. CrossHook only maintains its own detection and guidance layer. |
| H-M2 | **Detection code maintenance** — `host_command_exists()` and onboarding readiness must stay current with new distros and tool installation paths | 3   | 2   | **6** | Low    | §06      | `resolve_umu_run_path()` already probes ~20 candidate paths. New distros or tool relocations require path updates. Low severity — adds a few lines per new path.      |

### 3.3 UX Risks

| #    | Risk                                                                                                                                                                           | L   | I   | Score       | Rating | Evidence                     | Notes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --- | --- | ----------- | ------ | ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H-U1 | **[BIAS-COMPENSATED] 7-step prerequisite wall on bare immutable distros** — users without pre-installed gaming tools face a daunting setup before CrossHook is useful          | 4   | 4   | **16**      | High   | §07, §08 §1, §10 (Bias note) | This is the strongest argument against Host-Only. Anti-bundling bias in the research corpus may underweight this cost. On bare Fedora Atomic or vanilla NixOS, users must install Wine/Proton, gamescope, MangoHud, GameMode, winetricks, umu-launcher, and configure permissions — a 7-step process that many non-technical users will abandon. The counterpoint (§05, §08) that gaming-focused immutable distros pre-install these tools is valid but applies only to Bazzite/SteamOS/ChimeraOS, not to general-purpose immutable distros. |
| H-U2 | **First-run abandonment** — users who install CrossHook from Flathub and find it can't do anything without host tools may uninstall immediately                                | 4   | 3   | **12**      | Medium | §08 §1                       | No major Linux gaming launcher provides an in-app first-run wizard for bare-host onboarding. CrossHook's onboarding readiness checks (§06) are a partial solution but cannot install tools for the user. The gap between "installed the app" and "the app does something useful" is a conversion funnel problem.                                                                                                                                                                                                                             |
| H-U3 | **"Doesn't work on Steam Deck" perception** — SteamOS's immutable rootfs means `pacman` changes are wiped on updates; users may struggle to persistently install missing tools | 3   | 3   | **9**       | Medium | §07, §08 §7                  | Valve recommends Flatpak for additional apps on Steam Deck. Host tool persistence across SteamOS updates is fragile. However, SteamOS ships gamescope, MangoHud, and GameMode pre-installed. The gap is primarily umu-launcher and Proton version management — both of which Steam itself handles for Steam games, and CrossHook already discovers Steam's Proton installs.                                                                                                                                                                  |
| H-U4 | **Consistent mental model** — "everything runs on the host" is a simple, predictable model for users to understand                                                             | —   | —   | **Benefit** | —      | §08 §2                       | Users know: if a tool isn't installed on your system, CrossHook can't use it. No ambiguity about what's bundled vs. host. Error messages can clearly say "gamescope not found on your system — install it with [distro-specific command]."                                                                                                                                                                                                                                                                                                   |

### 3.4 Distribution Risks

| #    | Risk                                                                                                                                        | L   | I   | Score       | Rating | Evidence | Notes                                                                                                                                                                                                                                            |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ----------- | ------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| H-D1 | **Flathub acceptance likely — Lutris precedent** — CrossHook's permission profile mirrors Lutris, which is accepted                         | 2   | 3   | **6**       | Low    | §01, §05 | Lutris declares `--talk-name=org.freedesktop.Flatpak`, `--filesystem=home`, and similar permissions. It is accepted on Flathub. CrossHook's permission set is comparable. The Lutris precedent is strong but not guaranteed for a new app (§10). |
| H-D2 | **Post-CVE policy tightening** — CVE-2026-34078 may make Flathub reviewers scrutinize new `org.freedesktop.Flatpak` requests more carefully | 3   | 3   | **9**       | Medium | §05, §10 | The CVE is real (patched in Flatpak 1.16.4, April 2026). Behavioral prediction about reviewers is plausible but unverifiable. Existing apps like Lutris are unlikely to lose the permission, but new apps may face higher scrutiny.              |
| H-D3 | **Smallest package size** — no bundled tools means minimal download and install footprint                                                   | —   | —   | **Benefit** | —      | §02      | Current AppImage is ~50MB. Flatpak with GNOME runtime dependency but no bundled tools would be similarly modest (plus shared runtime). Best-in-class for download size.                                                                          |

### 3.5 Strategic Risks

| #    | Risk                                                                                                                                                                                                         | L   | I   | Score  | Rating  | Evidence             | Notes                                                                                                                                                                                                                                                                                                                                                     |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --- | --- | ------ | ------- | -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H-S1 | **[BIAS-COMPENSATED] Immutable distro growth undercuts the model** — if immutable distros reach 70% of Linux gaming (§07 projection), host tool installation becomes increasingly difficult for the majority | 3   | 4   | **12** | Medium  | §07, §10 (Bias note) | The 70% immutable projection is one researcher's subjective estimate, not data-driven (§10). However, the trend is real and accelerating. The counterpoint — gaming-focused immutable distros pre-install tools — limits the impact to ~10-20% of the immutable audience (general-purpose distro users). Score reflects the genuine but bounded audience. |
| H-S2 | **Competitive disadvantage vs. Bottles** — Bottles' "Flatpak-only, everything bundled" approach provides a frictionless first-run; CrossHook may seem less polished                                          | 3   | 2   | **6**  | Low     | §01, §03             | Bottles and CrossHook serve different purposes (Wine prefix manager vs. trainer orchestrator). Direct competition is limited. But user expectations for Flatpak gaming apps are set by Bottles' polish.                                                                                                                                                   |
| H-S3 | **umu-launcher consolidation is an opportunity** — as umu becomes the standard Proton orchestrator, CrossHook's single dependency on umu-run replaces dependencies on 3-4 separate tools                     | 3   | 0   | **0**  | Benefit | §07                  | umu-launcher consolidating the Proton launch chain reduces CrossHook's host dependency surface. If umu handles gamescope integration, MangoHud injection, and Proton management, CrossHook needs fewer host tool detections.                                                                                                                              |

### Strategy 3 Summary

| Dimension             | Top Risk Score | Count High+ |
| --------------------- | -------------- | ----------- |
| Technical             | 12 (Medium)    | 0           |
| Maintenance           | 6 (Low)        | 0           |
| UX                    | 16 (High)      | 1           |
| Distribution          | 9 (Medium)     | 0           |
| Strategic             | 12 (Medium)    | 0           |
| **Total High+ Risks** |                | **1**       |

**Assessment**: Host-Only has the cleanest risk profile with only one High risk (first-run prerequisite wall on bare immutable distros). Technical risks are low because the architecture is simple and well-proven — `platform.rs` already handles `flatpak-spawn --host` correctly. The primary vulnerability is the UX gap for users who can't easily install host tools, which can be substantially mitigated by a comprehensive onboarding wizard without changing the bundling strategy.

---

## Strategy 4: Hybrid/Extension-Based

> Use Flatpak extensions (VulkanLayer, SDK extensions) for GPU overlay tools; detect+prompt+guide for everything else; optionally build in lightweight features like Proton version management.

### 4.1 Technical Risks

| #    | Risk                                                                                                                                                                                                                                           | L   | I   | Score  | Rating   | Evidence                          | Notes                                                                                                                                                                                                                                                                                                                                                                                                         |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ------ | -------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E-T1 | **VulkanLayer extensions do not apply to host-launched games** — extensions mount into the sandbox runtime at `/usr/lib/extensions/vulkan/`; games launched via `flatpak-spawn --host` inherit the host's Vulkan layer path, not the sandbox's | 5   | 4   | **20** | Critical | §04, §09 (Contradiction 1.2), §10 | This is the single most important unresolved technical question. If VulkanLayer extensions cannot reach host-launched games, the extension-based approach for MangoHud and gamescope is invalid. The answer is almost certainly "no" based on Vulkan layer discovery mechanics (per-process `VK_LAYER_PATH`), but this has not been empirically tested for CrossHook's specific `flatpak-spawn --host` model. |
| E-T2 | **Extension version lag** — VulkanLayer extensions track the Flatpak runtime version, not upstream tool releases                                                                                                                               | 3   | 2   | **6**  | Low      | §01, §08                          | MangoHud and gamescope VulkanLayer extensions are built against specific runtime versions. Users wanting the latest MangoHud features may be stuck on an older extension version. Low impact because the host version (which IS what the game sees) is unaffected.                                                                                                                                            |
| E-T3 | **GameMode portal PID registration bug** — known issue with `org.freedesktop.portal.GameMode` PID mapping                                                                                                                                      | 3   | 2   | **6**  | Low      | §04                               | The PID registration bug affects both portal-based and direct GameMode invocation. Not specific to the hybrid strategy. Workaround exists in CrossHook's code.                                                                                                                                                                                                                                                |
| E-T4 | **Proton version management as built-in feature** — the only tool-like feature viable for building into CrossHook, but requires maintaining a download/extract/version-manage pipeline                                                         | 3   | 3   | **9**  | Medium   | §01, §04                          | CrossHook already discovers Proton installations. Adding download management means maintaining HTTP/archive code, tracking Proton-GE releases, and handling extraction to `~/.local/share/Steam/compatibilitytools.d/`. Bottles and ProtonUp-Qt demonstrate this is feasible but non-trivial. Scope creep risk: users will want umu-launcher, DXVK, and VKD3D management too.                                 |

### 4.2 Maintenance Risks

| #    | Risk                                                                                                                       | L   | I   | Score | Rating | Evidence    | Notes                                                                                                                                                                                                                                                                                                       |
| ---- | -------------------------------------------------------------------------------------------------------------------------- | --- | --- | ----- | ------ | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E-M1 | **Minimal bundling means minimal maintenance** — extensions are maintained by their upstream projects, not CrossHook       | 4   | 1   | **4** | Low    | §01, §03    | VulkanLayer extensions for MangoHud and gamescope are maintained by their respective projects. CrossHook only needs to declare them as optional dependencies in its manifest. Zero maintenance burden for CrossHook.                                                                                        |
| E-M2 | **Proton manager maintenance if built in** — tracking Proton-GE, Wine-GE, and umu-proton releases requires API integration | 3   | 3   | **9** | Medium | §04         | ProtonUp-Qt and ProtonPlus demonstrate the maintenance commitment: GitHub API integration, version parsing, archive extraction, and backward compatibility. If CrossHook builds this in, it's a permanent feature to maintain.                                                                              |
| E-M3 | **Detect+prompt+guide logic** — distro-specific install instructions must be kept current                                  | 3   | 2   | **6** | Low    | §06, §08 §1 | CrossHook's `build_umu_install_advice()` already provides distro-specific guidance. Expanding this to all tools requires maintaining per-distro install commands (apt, dnf, pacman, zypper, rpm-ostree, etc.) and updating when package names change. Low severity per change but scales with distro count. |

### 4.3 UX Risks

| #    | Risk                                                                                                                                                                                | L   | I   | Score       | Rating | Evidence                     | Notes                                                                                                                                                                                                                                                                                                                             |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ----------- | ------ | ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E-U1 | **Consistent "host tools" mental model with guided setup** — users understand all gaming tools run on their system; CrossHook helps them get set up                                 | —   | —   | **Benefit** | —      | §03, §08 §1                  | The "Detect + Prompt + Install" pattern is the universal approach across sandbox platforms (VS Code, Podman Desktop, macOS). Users learn: "CrossHook is the control panel; your system provides the tools." Error messages are clear and actionable.                                                                              |
| E-U2 | **[BIAS-COMPENSATED] First-run wizard still requires host tool installation** — even with excellent guidance, bare-distro users must install tools; some will abandon               | 4   | 3   | **12**      | Medium | §07, §08 §1, §10 (Bias note) | A great wizard reduces friction but doesn't eliminate it. On distros where `rpm-ostree install mangohud` takes 5 minutes and a reboot, some users will give up. The anti-bundling corpus underweights this cost. However, the alternative (bundling tools that can't reach host games) doesn't actually solve the problem either. |
| E-U3 | **Extension installation friction** — if CrossHook recommends VulkanLayer extensions, users must install them via Flatpak CLI or GNOME Software; no in-app install mechanism exists | 3   | 2   | **6**       | Low    | §01, §08 §2                  | `flatpak install org.freedesktop.Platform.VulkanLayer.MangoHud` is a single command but requires the user to leave the app. Could be mitigated by a "click to install" feature using `flatpak-spawn --host flatpak install`.                                                                                                      |

### 4.4 Distribution Risks

| #    | Risk                                                                                                                      | L   | I   | Score       | Rating | Evidence | Notes                                                                                                                                                                                       |
| ---- | ------------------------------------------------------------------------------------------------------------------------- | --- | --- | ----------- | ------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E-D1 | **Clean Flathub submission** — no bundled tools, standard extension dependencies, well-documented permission requirements | —   | —   | **Benefit** | —      | §05      | The manifest declares extensions as optional dependencies (Flatpak handles this natively). Permission requirements match Lutris's accepted set. The cleanest Flathub story of any strategy. |
| E-D2 | **Post-CVE scrutiny still applies** — `--talk-name=org.freedesktop.Flatpak` is still required regardless of strategy      | 3   | 3   | **9**       | Medium | §05, §10 | Same as H-D2. This risk is invariant across all four strategies because CrossHook fundamentally needs `flatpak-spawn --host` for game launches.                                             |

### 4.5 Strategic Risks

| #    | Risk                                                                                                                                                                          | L   | I   | Score       | Rating | Evidence | Notes                                                                                                                                                                                                                                             |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --- | --- | ----------- | ------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E-S1 | **Extension strategy depends on unproven assumption** — if VulkanLayer extensions CAN'T reach host-launched games (likely), the entire "extension-based" component is theater | 4   | 3   | **12**      | Medium | §04, §09 | If extensions don't work for host games, this strategy collapses to "Host-Only with better onboarding" — which is actually a fine outcome, just with less differentiation. The extension component is a nice-to-have, not a load-bearing element. |
| E-S2 | **Podman Desktop pattern aligns well** — thin GUI + host daemon delegation + first-run wizard is a proven Flatpak pattern                                                     | —   | —   | **Benefit** | —      | §03, §10 | CrossHook's architecture (thin UI, delegates to host tools via `flatpak-spawn --host`) most closely matches Podman Desktop's model, not Bottles'. This is the pattern Flathub reviewers are accustomed to accepting.                              |
| E-S3 | **Future-proofed for umu consolidation** — as umu-launcher absorbs tool orchestration, CrossHook's detect+guide layer naturally shrinks                                       | —   | —   | **Benefit** | —      | §07      | If umu-run handles gamescope wrapping, MangoHud injection, and Proton management, CrossHook's required host-tool surface shrinks to: (1) umu-run, (2) Steam. The hybrid strategy accommodates this evolution without wasted bundling investment.  |

### Strategy 4 Summary

| Dimension             | Top Risk Score | Count High+ |
| --------------------- | -------------- | ----------- |
| Technical             | 20 (Critical)  | 1           |
| Maintenance           | 9 (Medium)     | 0           |
| UX                    | 12 (Medium)    | 0           |
| Distribution          | 9 (Medium)     | 0           |
| Strategic             | 12 (Medium)    | 0           |
| **Total High+ Risks** |                | **1**       |

**Assessment**: The Hybrid/Extension-Based strategy has the best overall risk profile, with one Critical risk (VulkanLayer extensions not reaching host games) that, if confirmed, simply downgrades this strategy to an enhanced Host-Only approach rather than making it unviable. The "Detect + Prompt + Guide" core is a proven pattern across sandbox platforms. The optional Proton version manager adds user value without fundamental architectural risk. The strategy is the most future-proof, aligning with both umu-launcher consolidation and potential Flatpak portal improvements.

---

## Cross-Strategy Risk Comparison

### Heat Map

| Risk Dimension        | Full Bundle   | Partial Bundle | Host-Only   | Hybrid/Extension |
| --------------------- | ------------- | -------------- | ----------- | ---------------- |
| **Technical**         | Critical (25) | High (15)      | Medium (12) | Critical (20)\*  |
| **Maintenance**       | Critical (20) | Medium (12)    | Low (6)     | Medium (9)       |
| **UX**                | Medium (12)   | Critical (20)  | High (16)   | Medium (12)      |
| **Distribution**      | High (15)     | Medium (12)    | Medium (9)  | Medium (9)       |
| **Strategic**         | Medium (12)   | Medium (12)    | Medium (12) | Medium (12)      |
| **Total High+ Risks** | **7**         | **2**          | **1**       | **1**            |

_\* Hybrid's Critical technical risk (E-T1: VulkanLayer not reaching host games) downgrades the strategy to "enhanced Host-Only" if confirmed — it does not make the strategy unviable._

### Aggregate Risk Profile

| Strategy         | Max Risk Score | Avg of Top Risks | High+ Count | Structural Blockers                                                     |
| ---------------- | -------------- | ---------------- | ----------- | ----------------------------------------------------------------------- |
| Full Bundle      | 25             | 19.1             | 7           | 3 (gamescope impossible, winetricks mismatch, host exec still required) |
| Partial Bundle   | 20             | 14.0             | 2           | 0 (but UX uncanny valley is near-structural)                            |
| Host-Only        | 16             | 12.0             | 1           | 0                                                                       |
| Hybrid/Extension | 20             | 13.3             | 1           | 0 (Critical risk degrades gracefully)                                   |

---

## Risk Clusters

### Cluster 1: "Host Execution Is Non-Negotiable"

**Risks**: F-T1, F-T3, F-T4, P-T1, E-T1
**Pattern**: CrossHook launches games via `flatpak-spawn --host`. Games are host processes. Tools that must interact with the game process (MangoHud Vulkan injection, gamescope compositor wrapping) must also run on the host. No amount of sandbox-side bundling changes this.

**Implication**: This cluster eliminates Full Bundle as a viable strategy and severely limits what Partial Bundle can achieve. The tools that matter most for gaming (gamescope, MangoHud) are the ones that cannot be meaningfully bundled for CrossHook's architecture.

**Confidence**: High — derived from source code analysis (§04, §06) and Flatpak architecture documentation.

### Cluster 2: "Consistency Beats Capability"

**Risks**: P-U1, P-U2, P-S1, H-U4 (benefit)
**Pattern**: Users form mental models based on consistency. Partial bundling violates consistency by making some tools "just work" and others require installation. Host-Only provides consistency (nothing is bundled; everything needs host setup). The support ticket evidence from Heroic (#4791) confirms that inconsistency generates more confusion than absence.

**Implication**: If bundling cannot cover ALL user-facing tools (and Cluster 1 shows it cannot), partial bundling is worse than no bundling from a UX perspective. The optimal UX strategy is: commit to one model and execute it excellently.

**Confidence**: Medium-High — supported by Heroic evidence and UI theory, but not directly tested for CrossHook.

### Cluster 3: "The Immutable Distro Squeeze"

**Risks**: H-U1, H-U2, H-S1, E-U2
**Pattern**: Immutable distros make host tool installation harder. The Host-Only and Hybrid strategies both suffer from this. The squeeze is real but bounded: gaming-focused immutable distros (Bazzite, SteamOS, ChimeraOS) pre-install gaming tools, limiting the affected audience to general-purpose immutable distros (Fedora Atomic, vanilla NixOS).

**Implication**: The mitigation is the same regardless of strategy: a comprehensive onboarding wizard that detects missing tools and provides distro-specific installation guidance. This is valuable for ALL strategies and should be prioritized independently of the bundling decision.

**Confidence**: Medium — the trend is real; the affected audience size is uncertain.

### Cluster 4: "Flathub Gatekeeping"

**Risks**: F-D1, H-D2, E-D2
**Pattern**: All strategies require `--talk-name=org.freedesktop.Flatpak`. Post-CVE-2026-34078, this permission faces heightened scrutiny. The risk is invariant across strategies — no bundling choice eliminates it.

**Implication**: Flathub acceptance depends on CrossHook's justification quality, not its bundling strategy. A well-documented manifest explaining why host command execution is architecturally necessary (citing Lutris precedent) is the primary mitigation. Bundling MORE tools (Full or Partial) may actually increase scrutiny by expanding the manifest's attack surface.

**Confidence**: Medium — the CVE is real; the behavioral prediction about reviewers is plausible but unverifiable.

### Cluster 5: "Team Capacity Wall"

**Risks**: F-M1, F-M2, P-M1, P-M2, E-M2
**Pattern**: Every bundled component adds maintenance surface area. The relationship is multiplicative (tools x configs x distros), not additive. A small team cannot sustain security patch SLAs for multiple upstream projects simultaneously.

**Implication**: The bundling decision is fundamentally a project sustainability question (§08). Bundling should be considered only for tools where CrossHook can add genuine value over the host version — not as a convenience wrapper around tools the distro already manages.

**Confidence**: Medium-High — the mathematical scaling is certain; the team capacity constraint is assumed but reasonable for a small open-source project.

---

## Mitigation Strategies

### M1: Comprehensive Onboarding Wizard (Mitigates Cluster 3)

**Applicable to**: Host-Only, Hybrid/Extension
**Implementation**: Extend CrossHook's existing onboarding readiness checks (§06) into a full setup wizard that:

- Detects all available host tools with version information
- Provides distro-specific install commands (apt, dnf, pacman, rpm-ostree, etc.)
- Validates configuration after installation
- Offers a "one-click" install script via `flatpak-spawn --host bash -c '...'` where distro detection permits
- Handles the specific case of immutable distros (rpm-ostree overlay, Nix profile, etc.)

**Risk reduction**: H-U1 (16 → 9), H-U2 (12 → 6), E-U2 (12 → 6)
**Effort**: Medium — builds on existing `build_umu_install_advice()` infrastructure

### M2: Empirical VulkanLayer Testing (Resolves E-T1)

**Applicable to**: Hybrid/Extension
**Implementation**: Test whether a VulkanLayer extension installed in CrossHook's Flatpak runtime propagates to processes launched via `flatpak-spawn --host`:

1. Install `org.freedesktop.Platform.VulkanLayer.MangoHud` as a CrossHook extension
2. Launch a Vulkan game via `flatpak-spawn --host`
3. Check if MangoHud overlay appears on the game

**Expected result**: The extension does NOT propagate (based on Vulkan layer discovery mechanics), confirming E-T1 and downgrading the Hybrid strategy's extension component to informational ("we recommend you install MangoHud on your host").

**Risk reduction**: E-T1 resolves from "Critical unknown" to "confirmed architectural constraint" — removing uncertainty from the decision.
**Effort**: Low — single test session

### M3: Flathub Justification Document (Mitigates Cluster 4)

**Applicable to**: All strategies
**Implementation**: Prepare a manifest-accompanying document that:

- Explains CrossHook's orchestrator architecture
- Cites the Lutris precedent for `--talk-name=org.freedesktop.Flatpak`
- Documents each permission with its specific necessity
- Describes the `platform.rs` abstraction and `--clear-env` security measures
- References the custom env file handoff for sensitive values (§04, §06)

**Risk reduction**: F-D1 (15 → 9), H-D2 (9 → 6), E-D2 (9 → 6)
**Effort**: Low — documentation only

### M4: Version Detection + Host Preference (Mitigates H-T3)

**Applicable to**: Host-Only, Hybrid/Extension
**Implementation**: Extend `host_command_exists()` to also probe tool versions:

- `mangohud --version`, `gamescope --version`, `winetricks --version`
- Compare against known-compatible version ranges
- Warn users when host tool versions are too old or known-broken

**Risk reduction**: H-T3 (12 → 6)
**Effort**: Low-Medium — per-tool version parsing

### M5: Optional Built-In Proton Manager (Implements E-T4 Safely)

**Applicable to**: Hybrid/Extension
**Implementation**: Add a Proton version download/management feature that:

- Downloads Proton-GE or umu-proton to `~/.local/share/Steam/compatibilitytools.d/`
- Operates via `flatpak-spawn --host` to place files on the host filesystem
- Provides version selection and cleanup
- Is clearly labeled as optional — host-installed Proton is always preferred

**Risk reduction**: Reduces the most critical host dependency (Proton) to a guided in-app experience without bundling inside the sandbox.
**Effort**: Medium — requires GitHub API integration, archive handling, and version management UI

---

## Conclusions

### Strategy Viability Ranking

| Rank | Strategy             | Viability           | Rationale                                                                                                                                                                                                                                                                        |
| ---- | -------------------- | ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | **Hybrid/Extension** | **Recommended**     | Best risk profile. One Critical risk (E-T1) degrades gracefully to enhanced Host-Only. Aligns with proven Podman Desktop pattern. Future-proof for umu consolidation. Lowest maintenance burden among non-trivial strategies.                                                    |
| 2    | **Host-Only**        | **Viable**          | Simplest and lowest-maintenance. Only High risk (H-U1: first-run wall) is mitigable with onboarding wizard. Clean Flathub story. May be the correct starting point with Hybrid features added iteratively.                                                                       |
| 3    | **Partial Bundle**   | **Not recommended** | Uncanny valley UX risk (P-U1: score 20) is the worst user-facing risk across all strategies. Partial bundling cannot achieve "just works" because the most impactful tools (gamescope, MangoHud) must run on the host. Support burden evidence from Heroic confirms the pattern. |
| 4    | **Full Bundle**      | **Not viable**      | Three structural blockers (gamescope impossibility, winetricks version mismatch, host execution still required) cannot be mitigated. Seven High+ risks. The approach contradicts CrossHook's architectural identity.                                                             |

### Key Uncertainties Remaining

1. **VulkanLayer extension propagation to host games** — empirically testable (M2); answer determines whether Hybrid has any advantage over pure Host-Only for GPU overlay tools.
2. **Actual affected user base on bare immutable distros** — how many CrossHook Flatpak users will be on general-purpose immutable distros without pre-installed gaming tools? This determines the severity of the first-run wall.
3. **umu-launcher consolidation timeline** — if umu absorbs gamescope/MangoHud orchestration by 2027, the entire bundling question becomes moot. CrossHook would need only umu-run + Steam.
4. **Flathub reviewer posture post-CVE-2026-34078** — will new apps with `--talk-name=org.freedesktop.Flatpak` face significantly more scrutiny? Only answerable by submitting.

### The Bottom Line

The risk analysis converges on a clear finding: **CrossHook's architecture forces host execution for all gaming-relevant tools**. This is not a limitation to work around — it is a design strength that aligns with the orchestrator identity. The bundling decision should focus on improving the host-tool onboarding experience (onboarding wizard, distro-specific guidance, optional Proton management) rather than fighting the architecture by bundling tools that cannot reach host-launched games.

The single highest-value investment, regardless of strategy, is **M1: Comprehensive Onboarding Wizard**. It mitigates the primary UX risk (first-run wall) without introducing any of the maintenance, testing, or UX consistency risks that bundling creates.
