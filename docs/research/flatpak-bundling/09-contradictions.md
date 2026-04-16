# Phase 1 Crucible: Contradictions, Tensions, and Gaps

> **Purpose**: Map every contradiction, tension, and disagreement across the eight Phase 1 research files (01-historical through 08-negative-space) to surface the fault lines that the synthesis phase must resolve.
>
> **Method**: Each finding is cited by source file number (e.g., §01 = `01-historical.md`). Where two sources disagree, both positions are quoted or paraphrased with their reasoning so the synthesis team can weigh them fairly.

---

## 1. Direct Contradictions

These are cases where two or more Phase 1 files make claims that cannot both be true simultaneously, or recommend mutually exclusive actions.

### 1.1 Host Delegation as Legacy vs. Host Delegation as Architectural Necessity

| Position A (§01, §07)                                                                                                                                                                                                                     | Position B (§02, §04, §06)                                                                                                                                                                                                                                                |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Host delegation is a legacy pattern that Flathub is moving away from. The trend is toward self-contained Flatpaks with portal-based access. Future Flatpak evolution (systemd-appd, nested sandboxing) will further restrict host escape. | CrossHook's entire architecture routes through `flatpak-spawn --host` because games, Proton, gamescope, and MangoHud all run on the host. Bundling tools that must execute on the host is "architecturally circular." Every tool except GameMode requires host execution. |

**Why this matters**: If Position A is correct, CrossHook's architecture is on a deprecation path and must eventually internalize tools. If Position B is correct, bundling is a category error — CrossHook orchestrates host processes and always will.

**Resolution difficulty**: High. This is the central strategic question of the entire research effort.

### 1.2 VulkanLayer Extensions as Bundling Solution vs. VulkanLayer Extensions as Irrelevant

| Position A (§01, §03)                                                                                                                                                                                                                                                   | Position B (§04, §02)                                                                                                                                                                                                                                                                                                                             |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| VulkanLayer extensions (`org.freedesktop.Platform.VulkanLayer.MangoHud`, `.gamescope`, `.vkBasalt`) are the correct Flatpak-native mechanism for GPU overlay tools. §03 recommends them as Tier 1 strategy. §01 documents MangoHud and gamescope adopting this pattern. | Games run on the host, not inside CrossHook's sandbox. A VulkanLayer extension installed into CrossHook's runtime would be visible to CrossHook's own process, not to the game process launched via `flatpak-spawn --host`. §04 explicitly marks MangoHud and gamescope as "Should NOT bundle" because they must attach to the host game process. |

**Why this matters**: §03's Tier 1 recommendation (VulkanLayer extensions) is the most concrete "what to do" proposal in Phase 1. If §04 is correct that these extensions cannot reach host-launched games, the recommendation is technically invalid.

**Resolution difficulty**: Medium. This is empirically testable — does a VulkanLayer extension inside a Flatpak runtime propagate to processes launched via `flatpak-spawn --host`? The answer is almost certainly "no" based on how Vulkan layer discovery works (per-process `VK_LAYER_PATH`), which would confirm §04.

### 1.3 Immutable Distros as Strongest Bundling Argument vs. Immutable Distros Already Solved

| Position A (§07)                                                                                                                                                                                                                                                                                         | Position B (§05, §08)                                                                                                                                                                                                                                                                     |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Immutable distros (SteamOS, Bazzite, Fedora Atomic) are "the strongest argument for bundling" because users cannot easily install host packages. A bare Fedora Atomic user has no `apt install mangohud` escape hatch. §07 assigns 70% probability that immutable distros dominate Linux gaming by 2028. | §05 documents that Bazzite pre-installs all major gaming tools (MangoHud, gamescope, GameMode, umu-launcher) as part of its image. SteamOS ships everything needed. §08 §7 calculates that bundling on Steam Deck adds 500-600MB of tools already present, calling it a "redundancy tax." |

**Why this matters**: The immutable-distro argument is the most emotionally compelling case for bundling. But if every major immutable gaming distro already ships the tools, the argument applies only to non-gaming-focused immutable distros (vanilla Fedora Atomic, NixOS, etc.) — a much smaller audience.

**Resolution difficulty**: Medium. Requires quantifying the actual user base on "bare" immutable distros vs. gaming-focused ones (Bazzite, SteamOS, ChimeraOS). If 90%+ of immutable-distro gamers use gaming-focused distros, Position B largely wins.

### 1.4 Lutris as Precedent for Host Delegation vs. Lutris as Precedent for Internal Bundling

| Position A (§05, §06)                                                                                                                                                                                               | Position B (§01, §03)                                                                                                                                                                                                                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Lutris's Flatpak declares `--talk-name=org.freedesktop.Flatpak` and was accepted on Flathub. This is direct precedent that host-delegating game launchers can exist on Flathub. CrossHook can follow the same path. | Lutris bundles Wine runners internally (downloaded into `~/.var/app/net.lutris.Lutris/data/lutris/runners/wine/`). It uses host delegation for some operations but does NOT rely on host-installed Wine. The pattern is "bundle what you control, delegate what you can't." |

**Why this matters**: Both are technically true, but they support opposite conclusions. If Lutris is "proof that Flathub accepts host delegation," CrossHook can lean into `flatpak-spawn --host`. If Lutris is "proof that serious launchers bundle their core runtime," CrossHook should bundle Proton/Wine internally.

**Resolution difficulty**: Low. Both facts are correct — the question is which aspect of Lutris's strategy is relevant to CrossHook. Given that CrossHook explicitly does NOT manage Wine/Proton installations (it delegates to Steam or umu-launcher), the host-delegation precedent is more directly applicable.

---

## 2. Tensions

Both sides are true, but they pull in opposite directions, creating design trade-offs rather than factual disagreements.

### 2.1 Security Hardening vs. Functional Requirement

| Tension A (§05)                                                                                                                                                                                                                                                                 | Tension B (§04, §06)                                                                                                                                                                                                                                                                                                                     |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Post-CVE-2026-34078, Flathub reviewers are tightening sandbox escape permissions. `--talk-name=org.freedesktop.Flatpak` is the broadest possible sandbox escape — equivalent to running unsandboxed. Flathub may eventually restrict or flag this permission more aggressively. | CrossHook cannot function without `flatpak-spawn --host`. Its core value proposition is launching and managing host game processes with Proton. Removing this permission renders the app useless. §04 documents that CrossHook's sandbox is already "cosmetic" — it needs `--filesystem=home`, D-Bus access, and host command execution. |

**Implication**: CrossHook must hold a permission that is under increasing scrutiny. The app is inherently incompatible with a fully sandboxed model. This tension cannot be resolved by design choices — only by Flathub policy decisions.

### 2.2 Partial Bundling as Best Practice vs. Partial Bundling as Worst UX

| Tension A (§03)                                                                                                                                                                                                                                                                        | Tension B (§08)                                                                                                                                                                                                                                                                                                                         |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| §03 recommends a tiered approach: Tier 1 (Detect+Prompt+Guide for all deps), Tier 2 (VulkanLayer extensions for GPU tools), Tier 3 (Built-in Proton download manager as optional feature). This graduated approach matches VS Code, Podman Desktop, and other successful Flatpak apps. | §08 §3 identifies an "uncanny valley" where partial bundling creates the worst UX: some tools work out-of-the-box while others require manual host installation. Users cannot form a mental model of "what works" vs. "what needs setup." The inconsistency is more confusing than either "everything bundled" or "everything on host." |

**Implication**: The recommended strategy (partial/tiered) is also identified as the most confusing for users. Resolution requires either (a) making the tier boundaries invisible to users (everything "just works" or clearly explains why it doesn't), or (b) choosing one extreme.

### 2.3 Team Capacity vs. Bundling Maintenance Burden

| Tension A (§07, §08)                                                                                                                                                                                                                                     | Tension B (§02)                                                                                                                                                                                                                                                                                                |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| §07 projects that immutable distros will dominate, making host-tool installation harder. §08 notes the first-run experience on bare immutable distros requires 7 prerequisite steps without bundling. Users may abandon the app before completing setup. | §02 calculates that bundling 7+ tools requires tracking 7+ independent release cycles, security patches, and compatibility matrices. §08 §5 documents the testing matrix explosion: 240+ scenarios (6 tools x 8 host configs x 5 distros). A small team cannot maintain security patch SLAs for bundled tools. |

**Implication**: The user problem is real (hard setup on immutable distros), but the engineering solution (bundling) may be unsustainable for the team. This tension is about project sustainability, not technical architecture.

### 2.4 umu-launcher as Simplifier vs. umu-launcher as New Dependency

| Tension A (§01, §07)                                                                                                                                                                                                                                                                   | Tension B (§04, §08)                                                                                                                                                                                                                                                                                                                |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| umu-launcher is unifying the non-Steam Proton launch story. §07 assigns 80% probability that umu handles all Proton orchestration by 2027. This reduces CrossHook's tool surface — instead of managing Proton, gamescope, MangoHud, etc. individually, CrossHook delegates to umu-run. | §04 documents that umu-launcher is itself a host tool that must be installed. It doesn't eliminate the host-dependency problem — it consolidates it into a single critical dependency. On a bare system without umu-launcher, CrossHook still can't launch games. §08 notes umu is still maturing and may have its own instability. |

**Implication**: umu-launcher simplifies architecture but doesn't resolve the "what if it's not installed?" question. The dependency surface shrinks but doesn't vanish.

### 2.5 Flathub as Distribution Channel vs. Flathub Policy Constraints

| Tension A (§05, §07)                                                                                                                                                                                                                                                                                            | Tension B (§02, §05)                                                                                                                                                                                                                                                                                                                                       |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Flathub is the primary software distribution channel for Linux desktop apps, especially on immutable distros. §05 reports 438M downloads in 2025, growing rapidly. Not being on Flathub means not reaching most users. §07 projects Flathub becomes the default app store for gaming-focused immutable distros. | Flathub's policies are trending toward stricter sandboxing. CrossHook requires permissions (host command execution, broad filesystem access) that conflict with Flathub's direction. §02 argues that Flathub's model fundamentally doesn't fit host-orchestrating apps. §05 notes post-CVE scrutiny may further restrict `org.freedesktop.Flatpak` access. |

**Implication**: CrossHook needs Flathub for distribution but may face increasing friction with Flathub's security model. This is a business/ecosystem tension, not a technical one.

### 2.6 GameMode Portal Success vs. Portal Model Limitations

| Tension A (§01, §04)                                                                                                                                                                                                        | Tension B (§04, §05)                                                                                                                                                                                                                                                                                                         |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| GameMode's D-Bus portal (`org.freedesktop.portal.GameMode`) is the cleanest success story in Flatpak gaming. It works without sandbox escape, the Flatpak SDK auto-registers, and it's the model other tools should follow. | §04 documents a PID registration bug where Flatpak PID namespace isolation causes GameMode to receive the wrong PID. §05 confirms this is a known issue. The "clean" portal has real-world bugs. More importantly, no other gaming tool has achieved portal-level integration — GameMode is the exception, not the template. |

**Implication**: GameMode portal works but has bugs, and its success hasn't been replicated for MangoHud, gamescope, or other tools. Using GameMode as proof that "portals solve everything" overstates the evidence.

### 2.7 Bottles as Closest Analogue vs. Bottles as Wrong Analogue

| Tension A (§03)                                                                                                                                                                                                                                           | Tension B (§02, §04, §06)                                                                                                                                                                                                                                                                                                       |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| §03 identifies Bottles as "the closest analogue to CrossHook" — both are game launcher/managers with Proton integration. Bottles bundles Wine/Proton internally and works well as a self-contained Flatpak without `--talk-name=org.freedesktop.Flatpak`. | §02 and §06 argue CrossHook is fundamentally different from Bottles: CrossHook doesn't manage Wine installations, doesn't create isolated bottles/prefixes (it uses game-specific prefixes managed by Steam/Proton), and explicitly delegates all execution to host tools. CrossHook is an orchestrator, not a runtime manager. |

**Implication**: If Bottles is the right analogue, CrossHook should bundle more. If CrossHook is architecturally distinct (an orchestrator, not a manager), Bottles' approach is irrelevant. The analogy's validity depends on how you categorize CrossHook.

---

## 3. Unresolved Questions

Questions raised across Phase 1 that no file definitively answers.

### 3.1 What Happens When Flathub Tightens `org.freedesktop.Flatpak` Policy?

- §05 documents Lutris as precedent for acceptance.
- §05 also flags CVE-2026-34078 as a potential policy inflection point.
- §07 projects stricter sandboxing but no specific timeline.
- **No file models** what CrossHook would do if this permission were denied or deprecated. Would the app be removed from Flathub? Would an alternative host-communication mechanism exist?

### 3.2 What Is CrossHook's Actual Target User Distribution Across Distro Types?

- §07 makes projections about immutable distro adoption (70% by 2028).
- §08 argues Steam Deck dominates the immutable gaming segment.
- §05 notes Bazzite pre-installs gaming tools.
- **No file provides** actual data on CrossHook's current or projected user base by distro type. The bundling decision depends heavily on whether users are primarily on traditional distros (host tools easy), gaming-focused immutable distros (tools pre-installed), or bare immutable distros (tools unavailable).

### 3.3 Can CrossHook Detect and Guide Host Tool Installation Programmatically?

- §03 recommends "Detect+Prompt+Guide" as Tier 1 strategy.
- §06 documents CrossHook's existing `host_command_exists()` detection.
- **No file analyzes** whether CrossHook can reliably detect which package manager the host uses, whether it can invoke host package installation (e.g., via PackageKit portal or `flatpak-spawn --host pkcon install`), or what the UX for guided installation would actually look like across 5+ distro families.

### 3.4 What Is the Real-World Failure Rate When Host Tools Are Missing?

- §08 describes a 7-step first-run prerequisite chain.
- §02 argues most Linux gamers already have these tools.
- **No file provides** telemetry, user reports, or empirical data on how often CrossHook users actually encounter missing-tool failures. The entire bundling debate assumes this is a significant problem, but the magnitude is unquantified.

### 3.5 How Does pressure-vessel Interact with CrossHook's Flatpak Sandbox?

- §01 documents pressure-vessel as Valve's container-in-container approach for Steam Runtime.
- §04 mentions the execution chain passes through pressure-vessel for Steam games.
- **No file analyzes** the specific interaction: CrossHook (Flatpak sandbox) → `flatpak-spawn --host` → Steam → pressure-vessel (container) → game. Are there permission or environment-variable propagation issues in this double-container chain?

### 3.6 What Is the OGC's Likely Impact on Tool Standardization?

- §07 mentions the Open Gaming Collective (formed January 2026) as a potential unifying force.
- **No file provides** concrete analysis of OGC's roadmap, membership commitment, or likelihood of producing standards that would affect CrossHook's bundling decision.

### 3.7 Will Wine's Wayland Driver Eliminate gamescope's Necessity?

- §07 documents Wine 9.0-11.0 Wayland driver evolution.
- §01 notes gamescope provides session management and resolution control.
- **No file explicitly addresses** whether mature Wine Wayland support would reduce or eliminate gamescope's role, which would shrink CrossHook's dependency surface.

---

## 4. Confidence Gaps

Areas where Phase 1 research provides conclusions but with insufficient evidence or where confidence ratings diverge.

### 4.1 Flathub Policy Predictions

- §05 provides current policy state with High confidence.
- §07 projects future policy with Medium confidence but acknowledges "policy predictions are inherently speculative."
- **Gap**: The entire bundling strategy is sensitive to Flathub policy evolution, but predictions are Medium confidence at best. A Flathub policy change could invalidate either the "bundle" or "delegate" strategy.

### 4.2 Immutable Distro Adoption Rates

- §07 assigns 70% probability to immutable distro dominance by 2028.
- §05 provides current market data (Flathub downloads, distro adoption).
- **Gap**: The 70% figure is a projection, not data. CrossHook-specific user demographics are unknown. Decisions based on this projection carry significant uncertainty.

### 4.3 umu-launcher Maturity and Stability

- §01 and §07 treat umu-launcher as the emerging standard with High confidence.
- §08 notes umu is "still maturing" but doesn't quantify risk.
- **Gap**: CrossHook has already migrated to umu-launcher (per recent commits). The confidence in umu's stability is operationally important but not rigorously assessed in Phase 1.

### 4.4 VulkanLayer Extension Mechanics

- §01 and §03 recommend VulkanLayer extensions with Medium-High confidence.
- §04 implies they won't work for host-launched games but doesn't explicitly test this.
- **Gap**: The most concrete "what to do" recommendation (VulkanLayer extensions) has not been empirically validated for CrossHook's execution model. This is a testable gap that should be resolved before synthesis.

### 4.5 Security Patch SLA Feasibility

- §02 and §08 argue bundling creates unsustainable maintenance burden.
- §07 notes that bundling projects (Lutris, Bottles) have sustained this for years.
- **Gap**: No file quantifies the actual maintenance cost — hours per week, number of security patches per year for the tool set, or compares against CrossHook's team capacity. The "unsustainable" claim is asserted, not demonstrated.

### 4.6 First-Run Abandonment Rate

- §08 describes the 7-step prerequisite problem compellingly.
- §02 argues most users already have the tools.
- **Gap**: Neither position is backed by user data. The severity of the first-run problem is the strongest emotional argument for bundling, but it's entirely hypothetical in Phase 1.

### 4.7 CVE-2026-34078 Remediation Impact

- §05 flags this critical Flatpak sandbox escape vulnerability.
- §05 and §07 speculate it may trigger policy changes.
- **Gap**: The CVE is documented but its actual remediation and policy impact are unknown. It could be patched without policy changes, or it could trigger a wholesale review of `org.freedesktop.Flatpak` permissions. Phase 1 cannot resolve this — it depends on Flathub governance decisions.

---

## 5. Cross-Cutting Observations

### 5.1 The Fundamental Framing Disagreement

Phase 1 reveals two incompatible framings of CrossHook:

1. **CrossHook as Application** (§03, §07 lean this way): An app should be self-contained. Users expect Flatpak apps to "just work." Bundling is the Flatpak way. Compare to VS Code, Bottles, GIMP.

2. **CrossHook as Orchestrator** (§02, §04, §06 lean this way): CrossHook is a control plane for host processes. It doesn't run games — it tells the host to run games. Bundling tools inside the orchestrator is a category error, like bundling Docker images inside Portainer.

Every downstream disagreement traces back to this framing choice. The synthesis phase must resolve this before addressing individual tool decisions.

### 5.2 The Immutable Distro Paradox

The immutable-distro argument for bundling (§07) is simultaneously the strongest and weakest argument:

- **Strongest**: Users literally cannot install host tools on read-only root filesystems.
- **Weakest**: Every gaming-focused immutable distro (Bazzite, SteamOS, ChimeraOS) pre-installs gaming tools, and these are where gaming users actually are.

The argument only holds for users on non-gaming immutable distros (vanilla Fedora Atomic, NixOS) who want to game — a real but potentially small population.

### 5.3 The Sustainability Veto

§08's most important contribution is reframing the decision: "The bundling decision is not primarily a technical packaging question — it is a project sustainability question." Even if bundling were technically correct, a small team may not be able to sustain it. This is a constraint that overrides technical analysis.

### 5.4 The Testing Gap

No Phase 1 file empirically tested key technical claims:

- Do VulkanLayer extensions propagate to `flatpak-spawn --host` processes? (§03 vs. §04)
- Does `flatpak-spawn --host` correctly forward environment variables for MangoHud/gamescope? (§06 notes env-threading infrastructure exists, but doesn't confirm it works for all tools)
- What is the actual binary size of a Flatpak that bundles Proton + MangoHud + gamescope? (§02 estimates 800MB-1.5GB but doesn't measure)

Phase 2 should include empirical validation before the synthesis phase makes final recommendations.

---

## 6. Contradiction Severity Matrix

| #   | Contradiction/Tension                                   | Severity | Blocks Synthesis?                           |
| --- | ------------------------------------------------------- | -------- | ------------------------------------------- |
| 1.1 | Host delegation: legacy vs. necessity                   | Critical | Yes — central strategic question            |
| 1.2 | VulkanLayer: solution vs. irrelevant                    | High     | Yes — invalidates key recommendation        |
| 1.3 | Immutable distros: bundling argument vs. already solved | High     | Partially — depends on user demographics    |
| 1.4 | Lutris precedent: delegation vs. bundling               | Medium   | No — both facts are true, framing differs   |
| 2.1 | Security hardening vs. functional requirement           | High     | No — external constraint, not design choice |
| 2.2 | Partial bundling: best practice vs. worst UX            | High     | Yes — directly affects recommendation       |
| 2.3 | Team capacity vs. maintenance burden                    | Critical | Yes — sustainability veto                   |
| 2.4 | umu-launcher: simplifier vs. new dependency             | Medium   | No — reduces but doesn't eliminate problem  |
| 2.5 | Flathub: necessary channel vs. hostile policy           | High     | No — external constraint                    |
| 2.6 | GameMode portal: success vs. exception                  | Medium   | No — useful context but not blocking        |
| 2.7 | Bottles: closest analogue vs. wrong analogue            | Medium   | Partially — affects which patterns apply    |

**Synthesis-blocking items** (must resolve before final recommendation):

1. Is CrossHook an application or an orchestrator? (§5.1)
2. Do VulkanLayer extensions work for host-launched processes? (§1.2)
3. Can the team sustain bundled-tool maintenance? (§2.3)
4. Is partial bundling viable UX or uncanny valley? (§2.2)

---

## 7. Recommendations for Phase 2

1. **Empirically test VulkanLayer extension propagation** through `flatpak-spawn --host` to resolve §1.2.
2. **Quantify the immutable-distro user segment** — what fraction of CrossHook's target users are on bare (non-gaming) immutable distros? Resolves §1.3.
3. **Define CrossHook's identity explicitly** — orchestrator or application? This framing choice must precede all bundling decisions. Resolves §5.1.
4. **Estimate maintenance cost** — hours/week for tracking security patches across 7 tool release cycles. Compare against available team capacity. Resolves §2.3.
5. **Prototype the "Detect+Prompt+Guide" UX** — is it possible to guide host-tool installation from inside a Flatpak across major distro families? Resolves §3.3.
6. **Monitor CVE-2026-34078 remediation** — track Flathub's policy response. Resolves §4.7.
