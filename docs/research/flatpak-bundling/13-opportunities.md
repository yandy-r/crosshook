# Opportunities & Quick Wins: Flatpak Tool Bundling Synthesis

**Perspective**: Opportunity Synthesizer
**Date**: 2026-04-15
**Scope**: Actionable opportunities distilled from all 10 Phase 1 & Phase 2 research files, prioritized by effort-vs-impact
**Input files**: `01-historical.md` through `10-evidence.md`

---

## Executive Summary

After synthesizing ~5,000 lines of research across 10 files and accounting for the Crucible's evidence corrections (`10-evidence.md`), five categories of opportunity emerge. The single highest-ROI investment is **not** a bundling decision — it is a **first-run onboarding experience** that detects host tools and guides installation. This finding is robust across all research perspectives: the historical analysis (01), the analogical research (03), the negative-space analysis (08), and the archaeological dig (06) all converge on it independently. The Crucible confirms this convergence is genuine, not circular — each file arrives at the conclusion from different evidence bases.

**Key Crucible-corrected positions applied throughout this document:**

1. **VulkanLayer extensions are irrelevant for CrossHook.** Games launch on the host via `flatpak-spawn --host`. Extensions mount into the sandbox runtime, not the host process. Recommendations involving VulkanLayer extensions from `03-analogical` Tier 1 are **withdrawn**. (Crucible claim #19, Theme B)
2. **Podman Desktop, not Bottles, is CrossHook's closest architectural analogue.** Both are thin GUIs delegating to host daemons/tools via socket or `flatpak-spawn`. Bottles bundles Wine internally and does NOT use `flatpak-spawn --host` — the execution model diverges fundamentally. (Crucible §Assumption 2)
3. **Selective bundling (0-2 tools) is the real decision space**, not "all or nothing." The 800MB-1.5GB full-bundling estimate and 240+ testing matrix are straw men for a decision that involves at most 1-2 tools. (Crucible §Assumption 3)
4. **Background portal risk is likely misapplied.** Games are host processes; the Background portal monitors sandbox processes. No engineering work needed for this non-issue. (Crucible §Assumption 4)
5. **Performance claims (flatpak-spawn latency, seccomp overhead) are unreliable** and should not drive decisions without direct measurement. (Crucible Theme G)

---

## Opportunity Categories

### Category Legend

| Effort     | Definition                                                |
| ---------- | --------------------------------------------------------- |
| **Low**    | Days of work; uses existing infrastructure                |
| **Medium** | 1-2 weeks; requires new code but no architectural changes |
| **High**   | Weeks to months; new subsystem or significant refactoring |

| Impact     | Definition                                                           |
| ---------- | -------------------------------------------------------------------- |
| **High**   | Directly enables new users or eliminates a class of support issues   |
| **Medium** | Improves experience for existing users or future-proofs architecture |
| **Low**    | Nice-to-have; incremental improvement                                |

---

## 1. Quick Wins (Low Effort, High Impact)

### QW-1: Expand onboarding readiness checks with distro-specific install guidance

**Effort**: Low | **Impact**: High | **Confidence**: High

CrossHook already has `host_command_exists()` in `platform.rs` (line 526) and a readiness check system in `onboarding/readiness.rs` that detects Steam, Proton, and umu-run availability (06-archaeological). The infrastructure exists — it just needs to be extended.

**What to do:**

- Add readiness checks for gamescope, MangoHud, winetricks/protontricks, and GameMode
- For each missing tool, provide distro-specific install commands (detect distro via `os-release` through `flatpak-spawn --host cat /etc/os-release`)
- Include copy-to-clipboard functionality for install commands in the UI
- On immutable distros (Bazzite, Fedora Atomic, SteamOS), provide the correct method (`rpm-ostree install`, Flatpak install, or "already included" notification)

**Why this is high-impact:**

- The "7-step prerequisite" problem (08-negative-space §1) is the primary barrier for new Flatpak users on bare systems
- Every successful cross-domain analogue implements this pattern: macOS `xcode-select`, Podman Desktop onboarding, Android companion app detection (03-analogical)
- Bottles' biggest user complaints (#2008, #2801) are exactly this missing guidance (03-analogical §6)
- CrossHook's existing `host_command_exists()` makes detection trivial; the work is UI and install-command database

**Evidence basis**: 03-analogical (Pattern 1: Detect+Prompt+Install), 06-archaeological (readiness.rs infrastructure), 08-negative-space §1 (first-run gap), Crucible claim #24 (Medium-High confidence for this pattern). Four independent evidence chains — not circular.

---

### QW-2: Grey out unavailable features with explanatory tooltips and install links

**Effort**: Low | **Impact**: High | **Confidence**: High

When gamescope, MangoHud, or other optional tools are missing, the UI should clearly show the feature as disabled with a reason and remediation path — not hide it or fail silently.

**What to do:**

- In the optimization catalog UI, show unavailable optimizations as greyed-out rather than hidden
- Display a brief explanation: "gamescope not found on host — [How to install]"
- Link to a help page or show inline distro-appropriate install instructions
- Apply the same pattern to winetricks/protontricks in the prefix dependency UI

**Why this is high-impact:**

- Bottles' Flatpak learned this the hard way — v63.0 finally added "robust availability checks for Flatpak extensions" after years of silent failures and confused users (01-historical §2)
- The anti-pattern of silent failure (Firefox: no codec, just slow video) is the worst UX outcome (03-analogical §Anti-Patterns)
- CrossHook already has the `LaunchOptimizationDependencyMissing` validation error in the optimization resolution chain (06-archaeological §3, §4) — this just needs better UI surfacing

**Evidence basis**: 01-historical §2 (Bottles v63.0 fix), 03-analogical §Anti-Patterns (silent failure), 06-archaeological (existing validation infrastructure).

---

### QW-3: Leverage the GameMode D-Bus portal (zero bundling needed)

**Effort**: Low | **Impact**: Medium | **Confidence**: High

GameMode is the **only** tool in CrossHook's dependency set that works perfectly through a standard XDG Desktop Portal without sandbox escape (04-systems §3.5). This is a zero-bundling, zero-host-delegation path.

**What to do:**

- Verify CrossHook's GameMode integration uses the `org.freedesktop.portal.GameMode` portal when running as Flatpak
- If currently using `gamemoderun` via `flatpak-spawn --host`, consider switching to the portal API for the CrossHook process registration (the game process itself runs on the host and can use gamemoderun directly)
- Document this as the model for how CrossHook should handle tools when portals exist

**Why this matters:**

- GameMode portal is the cleanest success story in Flatpak gaming (01-historical §7, 04-systems §3.5)
- Demonstrates to Flathub reviewers that CrossHook uses portals where available, strengthening the justification for `flatpak-spawn --host` where portals don't exist
- Known caveat: PID registration bug where Flatpak PID namespace causes wrong PID (04-systems §3.5, Crucible claim #21 sidebar) — verify this doesn't affect CrossHook's use case

**Evidence basis**: 01-historical §7, 04-systems §3.5, Crucible Tier 1 claim #4. High-confidence primary sources.

---

### QW-4: Benchmark `flatpak-spawn --host` latency to close the evidence gap

**Effort**: Low | **Impact**: Medium | **Confidence**: High (that the gap exists)

The Crucible (10-evidence §Theme G) identified that **no direct benchmark exists** for `flatpak-spawn --host` latency. The ~50-150ms estimate in `08-negative-space` is extrapolated from unrelated measurements. This number appears in decision-making contexts but is unreliable.

**What to do:**

- Write a simple benchmark script: measure wall-clock time for `flatpak-spawn --host /bin/true` vs native `/bin/true` across 100 iterations
- Measure CrossHook's actual launch sequence: time each `flatpak-spawn` call in the game launch chain
- Publish results in the research docs to ground future performance discussions

**Why this matters:**

- Closes Evidence Gap #2 from the Crucible — the single most important unknown for performance-related bundling arguments
- If overhead is <50ms per call, the performance argument for bundling evaporates entirely
- If overhead is >200ms per call, it may justify optimizing the launch sequence (batching commands, reducing sequential calls)
- Takes minutes to implement; informs decisions worth weeks of engineering

**Evidence basis**: 10-evidence §Theme G (Gap #2), 08-negative-space §5 (unreliable estimates).

---

## 2. Strategic Opportunities (Medium-High Effort, High Impact)

### SO-1: Position CrossHook as the best Flatpak-first orchestrator (not bundler)

**Effort**: Medium (messaging and UX, not architecture) | **Impact**: High | **Confidence**: High

The Crucible's central finding is that CrossHook's identity question — "application or orchestrator?" (09-contradictions §5.1) — must be resolved before any bundling decision. The evidence overwhelmingly supports **orchestrator**.

**The case for orchestrator identity:**

- CrossHook routes **all** host tool access through `platform.rs` — it never executes Wine/Proton directly (06-archaeological, Crucible Tier 1 claims #1, #8, #11)
- Every tool except GameMode requires `flatpak-spawn --host` because games run on the host (04-systems §4)
- Podman Desktop (not Bottles) is the correct analogue: a thin GUI + host delegation (Crucible §Assumption 2)
- Bundling tools that must execute on the host is "architecturally circular" (04-systems §7)
- CrossHook already has the `platform.rs` abstraction that cleanly separates Flatpak/native paths — this is orchestrator architecture, not application architecture

**Strategic implication:**

- Stop evaluating CrossHook against Bottles (which bundles Wine) or GNOME Boxes (which bundles libvirt). These are applications, not orchestrators.
- Instead, benchmark against Podman Desktop (host daemon delegation), VS Code Flatpak (host tool delegation), and Lutris's `flatpak-spawn --host` path
- Communicate this identity clearly in Flathub submission: "CrossHook is a game trainer orchestrator that manages host-side game launches via Proton/Wine. It requires `org.freedesktop.Flatpak` because its core function is orchestrating host processes, analogous to Podman Desktop's relationship with the host container runtime."

**Differentiation opportunity**: No major Linux gaming launcher has fully embraced the orchestrator identity with excellent UX. Lutris's FAQ still recommends native over Flatpak. Heroic's Flatpak has runtime version pain. CrossHook can be the launcher that makes Flatpak-as-orchestrator work well.

**Evidence basis**: 04-systems (architecture analysis), 06-archaeological (platform.rs), 09-contradictions §5.1 (identity question), 10-evidence §Assumption 2 (Podman Desktop analogue). Crucible rates source code analysis as highest reliability.

---

### SO-2: Build Proton version management as a native feature

**Effort**: High | **Impact**: High | **Confidence**: Medium-High

This is the **only** "bundling" opportunity that the entire research corpus agrees on (04-systems §3.8, Crucible §Bundleability Matrix). It's not actually bundling an external tool — it's implementing download/extract functionality natively in Rust.

**What to do:**

- Implement GE-Proton and CachyOS-Proton download and extraction directly in CrossHook's Rust codebase
- Write to `~/.local/share/Steam/compatibilitytools.d/` (native Steam) or `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d/` (Flatpak Steam)
- Present available versions in the UI with install/remove/update capabilities
- Integrate with the existing `discover_compat_tools_with_roots()` scanner in `steam/proton.rs`

**Why this is strategic:**

- ProtonUp-Qt validates this pattern: a Flatpak app managing downloadable tools by writing to user data directories (01-historical §4)
- Eliminates a host tool dependency (protonup-qt) without requiring `flatpak-spawn --host`
- CrossHook's manifest already has `--filesystem=home` (covers native Steam paths)
- Aligns with umu-launcher's trajectory: as umu handles more orchestration, CrossHook's Proton management becomes the complementary download/version layer

**Caveat**: The `--filesystem=~/.var/app/com.valvesoftware.Steam:ro` permission is read-only. Writing compatibility tools for Flatpak Steam installations requires upgrading to `:rw` or using a different write path. This permission change needs Flathub justification.

**Evidence basis**: 04-systems §3.8 (bundleability matrix — only viable integration), 01-historical §4 (ProtonUp-Qt pattern), 06-archaeological §1 (proton.rs discovery infrastructure).

---

### SO-3: Align launch path with umu-launcher as the standard bridge

**Effort**: Medium | **Impact**: High | **Confidence**: High

umu-launcher is becoming the universal Proton bridge: Lutris 0.5.20 made it the default for GE-Proton (07-futurist §5.1), Heroic and Bottles support it, and the Crucible assigns 80% probability to umu handling all Proton orchestration by 2027.

**What to do:**

- CrossHook already supports umu-run with auto/force/skip preference (`settings/mod.rs`) and multi-strategy resolution in `resolve_umu_run_path()` (06-archaeological §2) — this is ahead of most competitors
- Deepen integration: use umu's game database for GAMEID matching (umu-protonfixes), which could automate per-game compatibility fixes
- On the Flatpak path, provide clear umu-run install guidance when it's missing (connects to QW-1)
- Monitor umu-launcher's own Flatpak packaging progress (issue #430) — when a well-packaged `org.openwinecomponents.umu.umu-launcher` Flatpak stabilizes, CrossHook can recommend it as a one-Flatpak install for the entire Proton stack

**Why this is strategic:**

- umu-launcher consolidates CrossHook's host dependency surface from many tools (Proton, pressure-vessel, Steam Runtime) into a single dependency
- As umu matures, CrossHook can delegate more launch mechanics and focus on its unique value: trainer orchestration
- The `--filesystem=xdg-data/umu:create` permission already in the manifest shows forward-thinking alignment

**Evidence basis**: 07-futurist §5 (umu trajectory), 06-archaeological §2 (existing umu integration), 01-historical §10 (umu Flatpak packaging), Crucible scenario matrix (80% probability).

---

### SO-4: First-run onboarding wizard (the "third option")

**Effort**: Medium-High | **Impact**: High | **Confidence**: High

The negative-space analysis (08 §Cross-Cutting Theme 4) identifies that the first-run experience is "the real battleground" — more important than the bundling decision itself. The analogical research (03 §2) identifies Podman Desktop's onboarding wizard as the strongest transferable pattern. CrossHook's existing `onboarding/readiness.rs` provides the backend foundation.

**What to do:**

- Build a step-by-step first-run wizard that:
  1. Detects the host environment: distro type (traditional/immutable/SteamOS), package manager, installed gaming tools
  2. Presents a checklist with status indicators (installed / missing / version info)
  3. For each missing tool, provides the correct install method for the detected distro
  4. Handles the immutable distro edge case: on Bazzite/SteamOS, most tools are pre-installed — confirm this and skip
  5. On bare Fedora Atomic, provide `rpm-ostree` commands or toolbox guidance
  6. Validates the setup before allowing game configuration
- Make the wizard re-runnable from settings (not just first launch)

**Why this is the "third option":**

- Neither bundling nor bare host-dependency — instead, actively guide users to correct host setup
- This is the least-discussed but potentially highest-ROI approach (08-negative-space §Recommendations #3)
- A great setup wizard with host-side tools beats a mediocre auto-bundle (08 §Cross-Cutting Theme 4)
- Podman Desktop proves this pattern works for Flatpak orchestrators (03-analogical §2)

**Implementation note**: Detecting the host distro from inside a Flatpak requires `flatpak-spawn --host cat /etc/os-release`, which CrossHook can already do. The complexity is in the distro-specific install command database and the UI.

**Evidence basis**: 08-negative-space §1, §Cross-Cutting Theme 4 (first-run as battleground), 03-analogical §2 (Podman Desktop), §7 (macOS Xcode prompt), 06-archaeological (readiness.rs foundation).

---

## 3. UX Opportunities

### UX-1: Consistent "all-host" mental model instead of partial bundling

**Effort**: Low (it's a design decision, not code) | **Impact**: High | **Confidence**: Medium-High

The negative-space analysis (08 §2) identifies the "uncanny valley of partial bundling" as a major UX risk: if some tools work out-of-the-box and others require host installation, users can't form a coherent mental model. Heroic's Flatpak demonstrates this exact problem (runtime version matching, host-vs-Flatpak MangoHud confusion).

**Recommendation**: Commit to the all-host-delegation model. CrossHook's architecture already implements this perfectly — every tool is invoked via `flatpak-spawn --host` or D-Bus portal. The opportunity is to make this explicit in the UX:

- Use consistent language: "CrossHook uses your system's gaming tools" (not "some tools are bundled")
- In the status panel, show all tools with their host status uniformly
- When something is missing, the message is always "Install X on your system" — no ambiguity about host vs. sandbox
- This eliminates the "which version is running?" confusion (08-negative-space §8)

**Crucible correction applied**: The original research in `03-analogical` recommends VulkanLayer extensions as Tier 1. The Crucible corrects this — VulkanLayer extensions mount into the sandbox runtime, not the host game process. Recommending them would create exactly the inconsistency this UX principle avoids.

**Evidence basis**: 08-negative-space §2 (uncanny valley), §8 ("why not both" trap), 10-evidence §Assumption 3 (straw man framing corrected).

---

### UX-2: Tool status dashboard in settings

**Effort**: Medium | **Impact**: Medium | **Confidence**: High

Extend the existing onboarding readiness concept into a persistent, always-accessible tool status dashboard.

**What to show:**

| Tool              | Status                       | Detail                 |
| ----------------- | ---------------------------- | ---------------------- |
| Proton            | Installed (GE-Proton 9-27)   | 3 versions available   |
| umu-run           | Installed (/usr/bin/umu-run) | v1.1.5                 |
| gamescope         | Not found                    | [Install for Arch]     |
| MangoHud          | Installed (v0.7.2)           | —                      |
| GameMode          | Active (via portal)          | Host daemon running    |
| winetricks        | Not found                    | [Install for Arch]     |
| Network isolation | Available                    | unshare --net works    |
| git               | Installed                    | Community taps enabled |

**Why this helps:**

- Surfaces the existing detection logic (all 13 tools documented in 06-archaeological) in a user-visible way
- Transforms the "install these 7 things" text wall into an interactive, progressive checklist
- Provides diagnostic value when users report issues ("what does your tool status screen show?")
- Snap's plug/slot interface model (03-analogical §9) provides the conceptual inspiration: named, typed capabilities with clear status

**Evidence basis**: 06-archaeological (all 13 tool detections documented), 03-analogical §9 (Snap capability model), 08-negative-space §1 (setup wizard opportunity).

---

### UX-3: Smart platform detection for Steam Deck / Bazzite / bare systems

**Effort**: Medium | **Impact**: Medium | **Confidence**: Medium

The immutable distro paradox (09-contradictions §1.3, §5.2) is that the strongest bundling argument (users can't install tools) is simultaneously weakest (gaming distros pre-install everything). The resolution is platform-aware behavior.

**What to do:**

- Detect SteamOS (read-only rootfs, all gaming tools present): skip onboarding for pre-installed tools; show "Your Steam Deck includes everything CrossHook needs"
- Detect Bazzite/ChimeraOS (immutable but gaming-ready): confirm pre-installed tools; flag anything missing with `rpm-ostree` guidance
- Detect bare Fedora Atomic/Silverblue (immutable, no gaming tools): full onboarding wizard with appropriate install methods
- Detect traditional distro (Arch, Fedora Workstation, etc.): standard package manager commands

**Why this matters:**

- Eliminates the "redundancy tax" concern for Steam Deck (08-negative-space §7: 500-600MB of already-present tools)
- Provides the right guidance for the right platform instead of one-size-fits-all
- The Crucible identifies user-base composition as the "single most important unknown" (10-evidence §Gap 1) — platform detection is the substitute for missing telemetry

**Evidence basis**: 08-negative-space §7 (Steam Deck redundancy), 09-contradictions §1.3, §5.2 (immutable distro paradox), 10-evidence §Gap 1 (user base unknown).

---

## 4. Architecture Opportunities

### AO-1: Document and protect the `platform.rs` abstraction gateway

**Effort**: Low | **Impact**: Medium | **Confidence**: High

The archaeological dig (06) and systems analysis (04) both identify `platform.rs` as CrossHook's most important architectural asset for Flatpak support. The Crucible rates source code analysis from these files as the highest reliability evidence in the entire corpus.

**What to do:**

- Add architectural decision record (ADR) documenting the "single abstraction gateway" pattern and why all host tool access must route through `platform.rs`
- Add a lint rule or CI check that flags direct `Command::new()` calls in `crosshook-core` that don't go through `host_command()` / `host_std_command()`
- Document the env-threading infrastructure (why `.env()` is silently dropped by `flatpak-spawn`, requiring `--env=K=V` args)
- This protects against future contributors accidentally bypassing the abstraction

**Why this matters:**

- `platform.rs` is the single reason CrossHook works in Flatpak at all. Every tool invocation, every env var, every path normalization routes through it (06-archaeological §Core Abstraction Layer)
- The env file handoff pattern (06-archaeological §Core Abstraction Layer, bullet 3) is non-obvious and critical for security — document it
- Future Flatpak improvements (fine-grained `flatpak-spawn` filtering per issue #5538) would be implemented here

**Evidence basis**: 06-archaeological §Core Abstraction Layer, §Architectural Patterns, 04-systems §1 (manifest analysis), Crucible reliability ranking (06, 04 are highest).

---

### AO-2: Prepare for fine-grained `flatpak-spawn` command filtering

**Effort**: Low (preparation only) | **Impact**: Medium (future) | **Confidence**: Medium

Flatpak issue #5538 proposes allowing apps to declare which specific host commands they can execute via `flatpak-spawn`, instead of the current blanket `org.freedesktop.Flatpak` permission. If implemented, this would dramatically reduce CrossHook's permission footprint and ease Flathub review.

**What to do now:**

- Maintain a definitive list of all host commands CrossHook invokes (06-archaeological documents 13 tools — this list should live in code or docs, not just research)
- Structure `platform.rs` so the host command list is centralized and enumerable (it largely is already)
- When submitting to Flathub, include this list in the review justification — it demonstrates CrossHook is a responsible user of `org.freedesktop.Flatpak`
- Monitor issue #5538 progress; when it lands, CrossHook can be an early adopter

**Why this matters:**

- The post-CVE-2026-34078 scrutiny of `org.freedesktop.Flatpak` (05-investigative §13) means CrossHook's Flathub submission will face heightened review
- Having a documented, bounded command set (not "arbitrary host execution") strengthens the case
- If filtering lands, CrossHook can declare: "I need `flatpak-spawn --host` for exactly these commands: proton, umu-run, gamescope, mangohud, kill, ps, cat, test, git, steam, winetricks, protontricks, lspci, unshare" — a much smaller attack surface than blanket host access

**Evidence basis**: 05-investigative §13 (Flathub policy), 06-archaeological (complete tool inventory), 10-evidence §Theme D (Flathub policy trajectory).

---

### AO-3: Investigate Protontricks-as-Flatpak for prefix dependency management

**Effort**: Medium | **Impact**: Medium | **Confidence**: Medium

The Crucible identifies Evidence Gap #5: no file explores whether Protontricks (`com.github.Matoking.protontricks` on Flathub) could serve CrossHook's prefix dependency management needs. This is a concrete gap worth closing.

**What to do:**

- Test whether CrossHook can invoke the Protontricks Flatpak via `flatpak run com.github.Matoking.protontricks` from inside its own Flatpak (requires appropriate D-Bus/process permissions)
- Compare this path against the current `flatpak-spawn --host winetricks/protontricks` path
- If viable, this eliminates a host tool dependency — users install Protontricks from Flathub (one click) instead of from their distro package manager

**Why this is interesting:**

- Protontricks is designed for Proton prefix management (better fit than winetricks for CrossHook's use case)
- Available on Flathub means it installs the same way on every distro, including immutable ones
- Could be recommended as a single Flatpak install in the onboarding wizard

**Evidence basis**: 10-evidence §Gap 5, 06-archaeological §7 (winetricks/protontricks detection chain).

---

## 5. Community & Ecosystem Opportunities

### CO-1: Build the Flathub submission case with the Lutris precedent

**Effort**: Low | **Impact**: High | **Confidence**: High

Lutris declares `--talk-name=org.freedesktop.Flatpak` and is accepted on Flathub (Crucible Tier 1 claim #5, verified from `flathub/net.lutris.Lutris` manifest). This is direct precedent.

**What to do:**

- Document CrossHook's permission justification modeled on Lutris's precedent
- Emphasize that CrossHook's `platform.rs` abstraction is more structured than Lutris's approach — CrossHook doesn't expose a general terminal, it invokes specific known commands
- Prepare a manifest review document listing: (a) every `flatpak-spawn --host` command, (b) why each is necessary, (c) which alternatives were considered and rejected (portals where available = GameMode)
- Highlight that CrossHook already uses the GameMode portal where it works — demonstrating preference for portals over host escape

**Why this matters:**

- Flathub acceptance is the gateway to the growing Flatpak user base (433.5M downloads in 2025, 07-futurist §10)
- The post-CVE environment means proactive justification beats reactive defense
- A well-prepared submission reduces review cycles and demonstrates good citizenship

**Evidence basis**: 01-historical §13 (Flathub policy), 05-investigative (Lutris precedent), 10-evidence Tier 1 claims #5, #12.

---

### CO-2: Contribute to Flatpak ecosystem improvements that benefit CrossHook

**Effort**: Low (engagement, not code) | **Impact**: Medium (long-term) | **Confidence**: Medium

Several upstream Flatpak improvements would directly benefit CrossHook. Engaging with these issues positions CrossHook as a constructive ecosystem participant.

**Upstream issues to monitor/engage:**

- **flatpak/flatpak#5538**: Fine-grained `flatpak-spawn` command filtering — CrossHook is an ideal test case
- **xdg-desktop-portal#536**: Gaming input portal — would reduce CrossHook's permission footprint
- **xdg-desktop-portal#1222**: Game Status Portal discussion — could formalize what CrossHook already does informally

**Why this matters:**

- CrossHook is small, but its use case (orchestrator needing bounded host access) is shared by Podman Desktop, VS Code Flatpak, and others
- Ecosystem improvement is more sustainable than per-app workarounds (08-negative-space §9)
- Engaging upstream demonstrates good faith for Flathub reviewers

**Evidence basis**: 05-investigative §13, 07-futurist §1.2 (portal development), 08-negative-space §9 (ecosystem effects).

---

### CO-3: Document CrossHook's Flatpak architecture publicly

**Effort**: Low-Medium | **Impact**: Medium | **Confidence**: Medium

No major Linux gaming launcher has published a detailed explanation of how it handles Flatpak sandboxing. CrossHook's `platform.rs` gateway pattern is genuinely well-designed (the Crucible rates 04-systems and 06-archaeological as highest reliability). Publishing this architecture would:

- Build credibility with Flathub reviewers
- Help other Flatpak orchestrator apps (the pattern is generalizable)
- Attract contributors who understand the constraints
- Pre-empt user confusion about why CrossHook needs broad permissions

**Evidence basis**: 06-archaeological (comprehensive architecture documentation), 04-systems (execution chain), Crucible reliability ranking.

---

## Prioritized Opportunity Matrix

| #        | Opportunity                                     | Effort      | Impact | Confidence  | Dependencies | Recommended Phase    |
| -------- | ----------------------------------------------- | ----------- | ------ | ----------- | ------------ | -------------------- |
| **QW-1** | Distro-specific install guidance in onboarding  | Low         | High   | High        | None         | Immediate            |
| **QW-2** | Grey out unavailable features with explanations | Low         | High   | High        | None         | Immediate            |
| **QW-3** | GameMode D-Bus portal verification              | Low         | Medium | High        | None         | Immediate            |
| **QW-4** | Benchmark flatpak-spawn latency                 | Low         | Medium | High        | None         | Immediate            |
| **CO-1** | Flathub submission case preparation             | Low         | High   | High        | None         | Immediate            |
| **SO-1** | Orchestrator identity positioning               | Medium      | High   | High        | None         | Near-term            |
| **UX-1** | All-host mental model commitment                | Low         | High   | Medium-High | SO-1         | Near-term            |
| **AO-1** | Protect platform.rs abstraction                 | Low         | Medium | High        | None         | Near-term            |
| **AO-2** | Prepare for flatpak-spawn filtering             | Low         | Medium | Medium      | AO-1         | Near-term            |
| **UX-2** | Tool status dashboard                           | Medium      | Medium | High        | QW-1         | Near-term            |
| **SO-4** | First-run onboarding wizard                     | Medium-High | High   | High        | QW-1, UX-2   | Medium-term          |
| **UX-3** | Smart platform detection                        | Medium      | Medium | Medium      | SO-4         | Medium-term          |
| **SO-2** | Native Proton version management                | High        | High   | Medium-High | AO-1         | Medium-term          |
| **SO-3** | umu-launcher alignment deepening                | Medium      | High   | High        | None         | Ongoing              |
| **AO-3** | Protontricks-as-Flatpak investigation           | Medium      | Medium | Medium      | None         | When resources allow |
| **CO-2** | Upstream ecosystem engagement                   | Low         | Medium | Medium      | CO-1         | Ongoing              |
| **CO-3** | Public architecture documentation               | Low-Medium  | Medium | Medium      | AO-1         | When resources allow |

---

## What We're NOT Recommending (And Why)

### Not recommended: VulkanLayer extensions for MangoHud/gamescope

`03-analogical` Tier 1 recommended this based on the Bottles analogue. The Crucible corrects this: VulkanLayer extensions mount into the sandbox runtime, but CrossHook's games run on the **host** via `flatpak-spawn --host`. The extension would apply to CrossHook's own Tauri/WebKitGTK window, not to the game process. This is architecturally irrelevant. (Crucible claim #19, §Assumption 2)

### Not recommended: Any tool bundling inside the Flatpak sandbox

The bundleability matrix (04-systems §7) is clear: no tool except Proton version management is viable for "bundling," and even that is better implemented as a native feature, not traditional bundling. The research corpus contains anti-bundling bias (6 of 8 Phase 1 files), but the Crucible confirms the core conclusion is sound — CrossHook's `flatpak-spawn --host` architecture makes sandbox-side bundling architecturally circular for tools that must interact with host game processes.

### Not recommended: "Why not both" (host + bundled fallback)

The negative-space analysis (08 §8) demonstrates this doubles the testing matrix, creates "which version is running?" confusion, and the maintenance cost of detection + fallback logic exceeds simply choosing one approach. The Lapce Flatpak fallback experience (08 §8) confirms the pattern fails in practice.

### Not recommended: Engineering work for Background portal risk

The Crucible identifies this as likely misapplied (10-evidence §Assumption 4). Games launched via `flatpak-spawn --host` are host processes. The Background portal monitors sandbox processes. The game PIDs live in the host PID namespace. No defensive engineering needed.

### Not recommended: Citing performance numbers in decision-making

The flatpak-spawn latency (~50-150ms) and seccomp overhead (3-19%) numbers are unreliable (Crucible Theme G). The latency estimate is extrapolated from unrelated measurements; the seccomp numbers measure games inside sandboxes, but CrossHook's games run on the host. QW-4 (benchmarking) should precede any performance-based decisions.

---

## Evidence Quality Notes

This synthesis explicitly accounts for the following Crucible corrections:

1. **Cross-citation inflation**: The Lutris #6144, gamescope #6, and 42% sandbox statistics each appear in 3-4 Phase 1 files. This document treats them as single data points, not independent corroboration.
2. **Anti-bundling corpus bias**: 6 of 8 Phase 1 files lean anti-bundling. This synthesis weights the pro-bundling arguments (immutable distro trend from 07-futurist) proportionally, but notes the Crucible's finding that gaming-focused immutable distros pre-install tools, weakening the argument.
3. **Selective bundling is the real option**: Recommendations are calibrated for 0-2 tool bundling decisions (the actual decision space), not the straw-man 7-tool scenario.
4. **Source reliability hierarchy**: Recommendations are primarily grounded in source code analysis (04, 06 — highest Crucible reliability) and primary sources (manifests, docs, CVEs), not ecosystem projections (07 — lowest reliability) or performance estimates (08 — unreliable numbers).

---

## Sources

This is a synthesis document. All sources are cited in the 10 input files:

- `01-historical.md` — Flatpak bundling precedents
- `02-contrarian.md` — Arguments against bundling
- `03-analogical.md` — Cross-domain analogies
- `04-systems.md` — Dependency graphs and permission models
- `05-investigative.md` — Current Flatpak gaming ecosystem
- `06-archaeological.md` — CrossHook's tool detection architecture
- `07-futurist.md` — Ecosystem trajectory projections
- `08-negative-space.md` — Blind spots and hidden costs
- `09-contradictions.md` — Phase 1 contradictions and tensions
- `10-evidence.md` — Evidence quality assessment (Crucible)
