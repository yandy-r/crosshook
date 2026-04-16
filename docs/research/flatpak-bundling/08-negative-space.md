# Negative Space Analysis: What's NOT Being Discussed About Flatpak Tool Bundling

> **Perspective**: Negative Space Analyst
> **Date**: 2026-04-15
> **Scope**: Blind spots, hidden costs, and overlooked factors in the CrossHook Flatpak bundling decision

## Executive Summary

The obvious bundling analysis frames the decision as "bundle tool X vs. don't bundle tool X" for each dependency. This report identifies nine categories of factors that this framing misses entirely. The most critical overlooked factors are: (1) the first-run experience gap on bare immutable distros, (2) the "uncanny valley" of partial bundling creating worse UX than no bundling at all, (3) the testing matrix explosion that a small project cannot sustain, and (4) the maintenance cliff where bundled tools become a security liability faster than they become a convenience.

**Key finding**: The bundling decision is not primarily a technical packaging question — it's a **project sustainability question** that should be evaluated against CrossHook's team size, release cadence, and support capacity.

---

## 1. The First-Run Experience Gap

### The Problem Nobody Discusses

What happens when a CrossHook Flatpak user has **nothing** installed on their host? No Wine, no Proton, no gamescope, no MangoHud — just a bare immutable distro with Flatpak support.

**Confidence**: High (multiple sources confirm immutable distro users face this exact scenario)

### Evidence

The immutable distro landscape (Bazzite, Fedora Silverblue/Kinoite, Vanilla OS, Universal Blue) is growing rapidly. These distros strongly prefer Flatpak for all application installation. The user profile is:

- Installed an immutable distro for stability/security
- Has Flatpak + Flathub configured
- Has **zero** host-side gaming tools installed
- May not even have a terminal-based package manager available (or it requires `rpm-ostree` / `transactional-update` which is unfamiliar)

On SteamOS (Steam Deck), the OS is immutable and any `pacman` changes are wiped on updates. Valve recommends installing additional applications via Flatpak to avoid issues. Users who install things outside of Flatpak "may get their Steam Deck into a bad state" and modifications "may be wiped with the next SteamOS update" ([Steam Deck FAQ — Steamworks Documentation](https://partner.steamgames.com/doc/steamdeck/faq)).

### What CrossHook Would Need to Tell Users Today (No Bundling)

On a bare immutable distro, the user would need to:

1. Install Wine or Proton (host-side, via `rpm-ostree` or distro tooling)
2. Install gamescope (host-side)
3. Install MangoHud (host-side)
4. Install GameMode (host-side)
5. Install winetricks or protontricks
6. Configure Flatpak permissions for CrossHook to access all of these via `flatpak-spawn --host`
7. Grant `org.freedesktop.Flatpak` portal access

This is a **7-step prerequisite installation** before CrossHook can do anything useful. Compare this to downloading a single AppImage that just works.

### The Guided Setup Wizard Opportunity

No major Linux gaming launcher currently provides an in-app first-run wizard that:

- Detects which tools are available on the host
- Explains what's missing and why it's needed
- Provides distro-specific installation instructions
- Validates the setup before allowing game configuration

**Bazzite** and **winesapOS** come closest — they ship as full gaming-ready distros with first-boot wizards ([winesapOS GitHub](https://github.com/winesapOS/winesapOS)). But these are _distros_, not _applications_. No Flatpak gaming app has solved the "bare host" onboarding problem.

**This is a gap CrossHook could fill regardless of the bundling decision.** A setup wizard that detects and guides is valuable whether tools are bundled or host-side.

### Blind Spot

The bundling discussion assumes the user already has a working gaming stack. For the growing immutable-distro audience, that assumption is increasingly wrong. **The first-run experience may matter more than whether tools are bundled or not.**

---

## 2. The Uncanny Valley of Partial Bundling

### The Problem

Partial bundling — where some tools work inside the sandbox and some require host installation — may create a **worse** user experience than fully committing to either approach.

**Confidence**: Medium (strong conceptual basis; real-world examples from Heroic confirm the pattern, but no direct study of user confusion rates)

### The Expectation Mismatch

Jeff Atwood's "uncanny valley of user interfaces" principle applies directly: users form a mental model based on their first interactions. If MangoHud works out of the box (bundled), but gamescope doesn't (host-required), the user doesn't think "gamescope requires host installation" — they think "gamescope is broken" ([Coding Horror — Avoiding the Uncanny Valley of User Interface](https://blog.codinghorror.com/avoiding-the-uncanny-valley-of-user-interface/)).

### Evidence from Heroic Games Launcher

Heroic's Flatpak demonstrates this exact problem:

- MangoHud and Gamescope must be installed as **Flatpak VulkanLayer extensions**, not system packages — but the error messages when they're missing don't make this clear to users
- Runtime version matching is critical: "a 24.x Gamescope won't be recognized by Heroic 2.18" — the Discovery store doesn't show versions, forcing CLI troubleshooting ([Heroic Issue #4791](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/4791))
- GameMode and gamescope can conflict: "enabling mangohud or gamemode might make gamescope not work" ([Heroic Wiki — Other Tools and Wrappers](<https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/How-To:-Other-Tools-and-Wrappers-(gamescope)>))
- Heroic had to add specific Flatpak-aware error messages (PRs #4588, #4729) to address support load from confused users

### The Support Ticket Prediction

With partial bundling, CrossHook would face a predictable support pattern:

| User Report                                 | Actual Cause                               | User's Mental Model                       |
| ------------------------------------------- | ------------------------------------------ | ----------------------------------------- |
| "MangoHud works but gamescope doesn't"      | Gamescope needs host install               | "CrossHook's gamescope support is broken" |
| "GameMode worked yesterday, broken today"   | Host OS updated, version mismatch          | "CrossHook update broke GameMode"         |
| "Nothing works after I changed permissions" | Flatseal removed `org.freedesktop.Flatpak` | "CrossHook is broken"                     |
| "Works on my desktop but not Steam Deck"    | Different host tool availability           | "CrossHook has a Steam Deck bug"          |

### Blind Spot

The bundling analysis evaluates each tool independently. But **consistency of experience matters more than individual tool availability**. A fully host-dependent approach with good error messages may generate fewer support tickets than a partially-bundled approach where some things "just work" and others don't.

---

## 3. The Testing Matrix Explosion

### The Problem

Every bundled tool multiplied by every distro configuration multiplied by every host tool version creates a combinatorial testing surface that small projects cannot cover.

**Confidence**: High (mathematical certainty of combinatorial growth; Flathub documentation confirms maintenance burden)

### The Math

CrossHook's potential bundled tools: Wine, Proton, gamescope, MangoHud, GameMode, winetricks/protontricks. That's ~6 tools.

Host configurations that matter:

- No host tools installed (bare immutable distro)
- Partial host tools (some installed, some not)
- Full host tools, same versions as bundled
- Full host tools, newer versions than bundled
- Full host tools, older versions than bundled
- Steam installed (native) with its own Proton
- Steam installed (Flatpak) with its own sandboxed Proton
- SteamOS (Steam Deck) with system gamescope

That's at minimum 8 host configurations x 6 tools = **48 test scenarios** just for tool availability. Add distro variants (Fedora, Ubuntu, Arch, Bazzite, SteamOS) and you're at **240+ test scenarios**.

### Evidence of This Being Unsustainable

The Flatpak documentation acknowledges: "while bundling is very powerful and flexible, it also places a **greater maintenance burden** on the application developer" and recommends keeping "the number of bundled modules as low as possible" ([Flatpak Dependencies Documentation](https://docs.flatpak.org/en/latest/dependencies.html)).

Flathub's own maintainer documentation warns about burnout: "Developing and maintaining software can be demanding, and maintainers may at times face time constraints, burnout, or shifting priorities" ([Flathub Maintenance Documentation](https://docs.flathub.org/docs/for-app-authors/maintenance)).

### Who Tests This?

CrossHook is a small project. Questions the bundling analysis doesn't ask:

1. **Who owns the CI matrix?** Does CrossHook CI test every bundled-tool × host-configuration combination?
2. **What's the regression detection time?** If a bundled gamescope update breaks interaction with host Steam, how long until someone notices?
3. **What's the user-reported-bug triage cost?** Every "it doesn't work" ticket requires determining: is the bug in CrossHook, the bundled tool, the host tool, or the interaction between them?

### Blind Spot

The bundling decision is often analyzed as a one-time packaging choice. In reality, it's an **ongoing testing and maintenance commitment** that scales combinatorially. The question isn't "can we bundle X?" but "can we sustain testing X across all environments indefinitely?"

---

## 4. Legal and Licensing Implications

### The Problem

Bundling open-source tools seems legally straightforward, but there are subtle obligations that are easy to overlook.

**Confidence**: Medium (licenses are clear in text; practical compliance obligations for Flatpak bundling are less discussed)

### License Summary

| Tool       | License                                                | Bundling Implications                                                                                                                                       |
| ---------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| MangoHud   | MIT                                                    | Freely bundleable. Include copyright notice. Minimal obligation.                                                                                            |
| GameMode   | BSD-3-Clause                                           | Freely bundleable. Include copyright + disclaimer. Minimal obligation.                                                                                      |
| Gamescope  | BSD-2-Clause                                           | Freely bundleable. Include copyright + disclaimer. Minimal obligation.                                                                                      |
| Winetricks | LGPL-2.1                                               | Bundleable, but must provide source for modifications. Dynamic linking preferred.                                                                           |
| Wine       | LGPL-2.1+ / GPL-2.0+                                   | Complex multi-license. Components under GPL cannot be statically linked into proprietary code. LGPL portions require source availability for modifications. |
| Proton     | BSD-3-Clause (Valve wrapper) + Wine LGPL + DXVK (Zlib) | Multi-license stack. Wine's LGPL propagates through.                                                                                                        |

### Flathub-Specific Requirements

Flathub mandates:

- All content must allow **legal redistribution** ([Flathub Requirements](https://docs.flathub.org/docs/for-app-authors/requirements))
- License must be correctly specified in MetaInfo
- License files for **each bundled module** must be installed to `$FLATPAK_DEST/share/licenses/$FLATPAK_ID`
- No end-of-life runtimes or high-risk dependencies with network permissions
- All dependencies must be specified as manifest sources with **publicly accessible URLs** (no vendored binaries)
- Everything must build from source

### The Subtle LGPL Issue

Wine's LGPL licensing was specifically chosen so proprietary software could use Wine libraries, but with the requirement that modifications be shared back ([Wine License Discussion — WineHQ Forums](https://forum.winehq.org/viewtopic.php?t=10117)). If CrossHook bundles Wine and makes any modifications (patches, configuration changes), those modifications must be made available under LGPL terms.

There's also historical confusion in the Flatpak ecosystem itself: flatpak-builder's own licensing has been debated, with source files declaring "Lesser General Public License version 2 or any later version" — a license version that technically doesn't exist ([flatpak-builder Issue #34](https://github.com/flatpak/flatpak-builder/issues/34)).

### Blind Spot

The permissive licenses (MIT, BSD) make most tools easy to bundle. But **Wine/winetricks LGPL obligations** require ongoing compliance tracking, and **Flathub's per-module license installation requirement** adds packaging complexity that scales with bundled tool count. This is not a blocker, but it is hidden maintenance cost.

---

## 5. Performance Overhead — The Numbers Nobody Quotes

### The Problem

Performance discussions focus on "Flatpak vs. native" in general terms. The specific overhead of CrossHook's usage pattern — a launcher that **frequently spawns processes** via `flatpak-spawn --host` — is rarely quantified.

**Confidence**: High (benchmark data exists for sandbox overhead; Medium for flatpak-spawn specifically, as no dedicated benchmarks exist)

### Sandbox Startup Overhead

Measured overhead for Flatpak process startup ([Flatpak Issue #2275](https://github.com/flatpak/flatpak/issues/2275)):

| Operation         | Native | Flatpak | Overhead |
| ----------------- | ------ | ------- | -------- |
| `/bin/true`       | 0.001s | 0.141s  | **141x** |
| Vim (no config)   | 0.004s | 1.152s  | **288x** |
| Vim (with config) | 0.060s | 1.206s  | **20x**  |

The **irreducible containerization overhead is ~140-150ms per process spawn**. This is the floor — it cannot be optimized away.

### Seccomp Filter Overhead for Games

The seccomp sandbox filter imposes measurable overhead on CPU-intensive game workloads ([Flatpak Issue #4187](https://github.com/flatpak/flatpak/issues/4187)):

| Benchmark                              | With Seccomp | Without | Overhead |
| -------------------------------------- | ------------ | ------- | -------- |
| Geekbench (single-core)                | 1286         | 1377    | **7%**   |
| Shadow of Tomb Raider (FPS)            | 123          | 127     | **3%**   |
| Shadow of Tomb Raider (Max CPU Render) | 435          | 517     | **19%**  |
| Elemental Demo (AVG FPS)               | 52           | 59      | **12%**  |

### What This Means for CrossHook

CrossHook's architecture involves:

1. User clicks "Launch" in the Tauri UI
2. CrossHook spawns trainer process (via Proton/Wine)
3. CrossHook spawns game process (or signals Steam)
4. Optional: MangoHud overlay injected
5. Optional: GameMode activated
6. Optional: gamescope compositor started

Each of these spawns, if routed through `flatpak-spawn --host`, adds the ~140-150ms D-Bus portal overhead. For a launch sequence involving 3-4 tool invocations, that's **~0.5-0.6 seconds of pure overhead** before any tool actually starts executing.

### Memory Overhead

Per-Flatpak overhead is modest:

- D-Bus proxy: ~0.5-1 MB RAM per Flatpak instance
- bwrap sandbox process: ~0.25 MB RAM per instance

Bundled tools add their binary size to the Flatpak package (but share runtimes). Host tools have zero Flatpak-side memory cost.

### `flatpak-spawn --host` Latency — The Missing Benchmark

No public benchmarks exist for `flatpak-spawn --host` latency specifically. The mechanism involves:

1. Application inside sandbox sends D-Bus message to Flatpak portal
2. Portal validates permissions (checks `org.freedesktop.Flatpak` access)
3. Portal spawns process on host
4. stdout/stderr piped back through portal

This is architecturally similar to the 140ms containerization overhead measured above, but in reverse (sandbox → host instead of host → sandbox). The actual latency is likely in the **50-150ms range per invocation** based on the D-Bus round-trip involved.

### Blind Spot

Bundling tools would eliminate `flatpak-spawn --host` overhead for bundled tools, but those tools would then run **inside** the sandbox with seccomp overhead. The performance tradeoff is: **~150ms per spawn (host path) vs. ongoing 3-19% CPU overhead (sandboxed path)**. For short-lived tool invocations (winetricks, one-shot configs), host spawning wins. For long-running processes (gamescope, game itself), seccomp overhead matters more. **Nobody discusses this tradeoff explicitly.**

---

## 6. The Maintenance Cliff

### The Problem

Bundled tools are frozen at their build-time version. The gap between upstream releases and Flatpak updates creates a **security and compatibility window** that grows with each bundled tool.

**Confidence**: High (well-documented pattern across the Flatpak ecosystem)

### The Security Patch Delay

Traditional package managers update a shared library once, fixing every application that uses it. Flatpak bundling breaks this model:

> "Because Snap and Flatpak applications are often distributed with their own libraries, security patches that apply to a certain library may not be immediately propagated to every Snap or Flatpak containing that library." ([machaddr.substack.com](https://machaddr.substack.com/p/snap-or-flatpak-on-linux-why-you))

> "Flathub apps often bundle runtimes with outdated libraries, even when fixed upstream months earlier. Users of those apps remain exposed because the sandboxed apps include vulnerable binaries frozen in time." ([Linux Journal — When Flatpak's Sandbox Cracks](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal))

### The Bundling Founder's Own Warning

This problem was recognized from the very beginning of the bundling model (2011):

> "Another problem is with security (or bugfix) updates in bundled libraries. With bundled libraries its much harder to upgrade a single library, as you need to find and upgrade each app that uses it."

### CrossHook's Specific Risk

If CrossHook bundles Wine/Proton and a critical Wine security vulnerability is disclosed:

| Scenario                                                  | Time to User Protection                                 |
| --------------------------------------------------------- | ------------------------------------------------------- |
| Host-side Wine (distro package)                           | Hours to days (distro maintainer pushes update)         |
| Bundled Wine in CrossHook Flatpak                         | Days to weeks (CrossHook team must rebuild, test, push) |
| Bundled Wine in CrossHook Flatpak (team busy/unavailable) | Weeks to months                                         |

This is compounded by CrossHook being a small project. There's no dedicated security response team. A Wine CVE disclosed on a Friday evening could remain unpatched in the CrossHook Flatpak for weeks.

### Compare: AppImage Distribution

CrossHook's AppImage distribution doesn't bundle Wine or gaming tools — it uses whatever the host provides. This means:

- No security patch delay for tools
- No version mismatch between bundled and host tools
- Users get tool updates immediately from their distro

**The Flatpak bundling question is specific to the Flatpak distribution channel**, not CrossHook in general. This asymmetry is rarely discussed.

### Blind Spot

The maintenance cliff isn't about whether you _can_ keep bundled tools updated — it's about what happens when you _can't_ (team capacity, competing priorities, vacations, burnout). **The question is: what's CrossHook's maximum acceptable patch delay, and can the team guarantee that SLA for every bundled tool?**

---

## 7. Steam Deck: The Redundancy Tax

### The Problem

Steam Deck is a primary target for CrossHook. SteamOS already includes all the gaming tools CrossHook might bundle. Bundling creates pure redundancy on this platform.

**Confidence**: High (SteamOS contents are well-documented)

### What SteamOS Already Provides

SteamOS ships with:

- **Gamescope** (the entire desktop is gamescope)
- **MangoHud** (accessible via Steam overlay settings)
- **GameMode** (integrated with Steam)
- **Wine/Proton** (multiple versions via Steam)
- **Winetricks/Protontricks** (available or easily installable)

All of these are native, optimized for the hardware, and maintained by Valve. Bundling any of them in a CrossHook Flatpak means shipping redundant copies of tools that are already present and working.

### The Disk Space Impact

Steam Deck storage is severely constrained:

- **64GB model**: ~10-11GB consumed by SteamOS, leaving ~40GB for user data. Of that, Proton compatibility data, shader caches, and Flatpak runtimes can easily consume 10-15GB, leaving **~25GB for actual games** ([Steam Community — 64GB storage discussion](https://steamcommunity.com/app/1675200/discussions/0/3385030647948351716/))
- Flatpak installations on SteamOS live under `/home/.steam` via OverlayFS, directly competing with game storage
- One user reported Flatpak apps consuming **7GB** on their 64GB model, contributing to a boot failure when space ran out ([Steam Community — Storage overload](https://steamcommunity.com/app/1675200/discussions/0/3274688652643211345/))

If CrossHook bundles gamescope (~50MB), MangoHud (~20MB), GameMode (~5MB), and Wine (~500MB+), that's potentially **500-600MB of redundant tools** on a device where every megabyte of game storage matters.

### The Version Conflict Risk

Worse than redundancy is version conflict. SteamOS's system gamescope is tightly integrated with the compositor — it literally _is_ the display server. A bundled gamescope at a different version could behave differently from the system gamescope that games expect, creating subtle rendering or performance issues.

Heroic's Flatpak encountered exactly this: "the Flatpak Gamescope version must match the version of the natively installed Gamescope. If these versions do not match, users may experience increased crashing in other Flatpaks" ([Heroic Wiki — Steam Deck](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Steam-Deck)).

### Blind Spot

The bundling analysis optimizes for the "bare distro" user. But **Steam Deck users are likely a large portion of CrossHook's audience**, and for them bundling is **pure cost with zero benefit**. Any bundling decision must account for this platform-specific redundancy, ideally by detecting SteamOS and skipping bundled tools entirely — which brings us to the next problem.

---

## 8. The "Why Not Both" Trap

### The Problem

The "detect host tools first, fall back to bundled" approach sounds elegant. In practice, it doubles complexity, doubles the bug surface, and creates user confusion about which version is actually running.

**Confidence**: Medium (strong reasoning, confirmed by Lapce Flatpak fallback issues; limited CrossHook-specific evidence)

### Evidence: Lapce's Flatpak Fallback

The Lapce editor encountered this exact problem in its Flatpak distribution. When users denied `org.freedesktop.Flatpak` portal access (e.g., via Flatseal), `flatpak-spawn --host` became unavailable, and the terminal feature broke entirely instead of falling back gracefully ([Lapce Issue #1133](https://github.com/lapce/lapce/issues/1133)).

The proposed fix — "fallback to default when flatpak-spawn --host is not available" — required detecting:

1. Whether we're inside a Flatpak sandbox
2. Whether the Flatpak portal is accessible
3. Whether the specific D-Bus permission is granted
4. Whether the host tool exists and is the right version
5. Whether the bundled fallback exists
6. Which version to prefer when both are available

### The Dual-Path Problem for CrossHook

If CrossHook implements "detect host, fall back to bundled":

```
User launches game →
  Is gamescope needed? →
    Is host gamescope available? →
      Is host gamescope version compatible? →
        Use host gamescope
      Else →
        Is bundled gamescope available? →
          Use bundled gamescope (inside sandbox, with seccomp overhead)
        Else →
          Launch without gamescope
    Else →
      Is bundled gamescope available? →
        Use bundled gamescope
      Else →
        Launch without gamescope
```

This decision tree exists for **every optional tool**. Multiply by 5-6 tools and you have a combinatorial explosion of code paths.

### The "Which Version Is Running?" Confusion

When something goes wrong, the user doesn't know:

- "Am I using host MangoHud or bundled MangoHud?"
- "My MangoHud shows version X but the host has version Y — which one is CrossHook using?"
- "I updated MangoHud on my system but CrossHook still shows old behavior" (because it fell back to bundled)

This is a **support nightmare** where every bug report requires first determining which code path was taken.

### The Testing Multiplication

The "both" approach doesn't halve the testing matrix — it **doubles** it:

- Test with host tool only
- Test with bundled tool only
- Test with both available (does the detection correctly prefer host?)
- Test with host tool broken/incompatible (does fallback work?)
- Test with bundled tool broken (does it gracefully degrade?)
- Test with neither available
- Test when permissions change mid-session (Flatseal)

### Blind Spot

"Why not both?" is a natural human response to binary choices, but in software engineering it often creates the worst of both worlds. **The maintenance cost of detection + fallback logic may exceed the maintenance cost of simply choosing one approach and documenting it well.**

---

## 9. Community and Ecosystem Effects

### The Problem

If CrossHook bundles tools, it reduces pressure on distributions and upstream projects to fix packaging issues, potentially fragmenting the Linux gaming ecosystem.

**Confidence**: Low (speculative; limited direct evidence for CrossHook's impact given its project size)

### The Broader Fragmentation Pattern

The Linux packaging ecosystem is already dealing with fragmentation fallout:

> "Developers feel the split too, and it's not just philosophical. Shipping in two formats can mean duplicating build pipelines, support documentation, and bug triage work." ([XDA — Linux's app problem](https://www.xda-developers.com/linuxs-app-problem-app-stores-refuse-merge/))

> "The fragmentation also blurs accountability in a uniquely frustrating way. If an app update breaks something, users aren't sure whether to blame the app, the store, the sandbox, or the distro integration layer." ([XDA](https://www.xda-developers.com/linuxs-app-problem-app-stores-refuse-merge/))

### The Positive Counter-Argument

Valve's endorsement of Flatpak (Steam Frame, Steam Machine) suggests the ecosystem is converging:

> "A single, reliable target for application distribution significantly lowers the development burden. Developers can focus on building great apps and games, knowing they can reach a vast Linux audience through Flathub." ([It's FOSS — Steam Frame and Flatpak](https://itsfoss.gitlab.io/post/steam-frame-machine-flatpaks-desktop-linux/))

If the ecosystem is converging on Flatpak, then participating in that ecosystem (including bundling where appropriate) is alignment, not fragmentation.

### The Incentive Misalignment

If CrossHook bundles Wine, it removes an incentive for immutable distros to make Wine easily available to Flatpak apps through better portal/extension mechanisms. CrossHook would be working around a platform limitation rather than pressuring the platform to fix it.

However, CrossHook is a small project. Its bundling decisions are unlikely to influence Flatpak platform development. The practical question is: **should CrossHook wait for the ecosystem to solve this, or ship a working solution now?**

### Blind Spot

The ecosystem effect is real but CrossHook's influence is small. **The more relevant community question is: does CrossHook bundling align with or diverge from what Heroic, Lutris, and Bottles are doing?** Diverging from the established launcher pattern creates a lonely support burden.

---

## Cross-Cutting Themes

### Theme 1: The Sustainability Question

Nearly every negative space points back to the same core issue: **can a small team sustain the ongoing cost of bundling?** The initial packaging work is a one-time cost. The testing, updating, security patching, and support are perpetual.

### Theme 2: Consistency Over Optimization

The uncanny valley problem, the "why not both" trap, and the testing matrix explosion all argue for the same principle: **pick one approach and execute it well, rather than optimizing each tool independently.** A consistent experience (even if imperfect) generates fewer support tickets than an inconsistent one.

### Theme 3: Platform-Specific Strategies May Be Required

Steam Deck (everything pre-installed), bare immutable distro (nothing installed), and traditional distro (mix of host tools) are such different environments that a single bundling strategy may not serve all three. This argues for **runtime detection and adaptation** (which collides with Theme 2's consistency principle — a genuine tension with no easy resolution).

### Theme 4: The First-Run Experience Is the Real Battleground

Whether tools are bundled or host-side, the **first-run experience** is what determines user success or abandonment. A great setup wizard with host-side tools beats a mediocre auto-bundle. A zero-config bundle beats a "install these 7 things" text file. **The UX wrapper matters more than the packaging strategy.**

---

## Recommendations for the Synthesis Phase

1. **Evaluate bundling as a maintenance commitment, not a packaging decision.** Calculate ongoing cost, not just initial setup cost.

2. **Weight the Steam Deck redundancy problem heavily.** If Steam Deck is a primary audience, bundling penalizes them to benefit a different audience.

3. **Consider the "setup wizard" path as a third option.** Neither bundling nor bare host-dependency — instead, actively guide users to correct host setup. This is the least-discussed but potentially highest-ROI approach.

4. **If bundling, bundle everything or nothing.** Partial bundling creates the worst UX. The uncanny valley is more damaging than either extreme.

5. **If host-dependent, invest in detection and error messaging.** Heroic's experience shows that clear, Flatpak-aware error messages dramatically reduce support load.

6. **Establish a security patch SLA before bundling.** If the team can't commit to patching bundled tools within N days of upstream releases, bundling is a liability.

---

## Search Queries Executed

1. `Flatpak first run experience bare system no dependencies installed user onboarding 2025 2026`
2. `Flatpak bundling tools testing matrix explosion maintenance burden small projects 2025`
3. `Flatpak sandbox overhead performance latency flatpak-spawn --host benchmarks`
4. `Flathub licensing requirements bundled components LGPL Wine redistribution policy 2025`
5. `Steam Deck SteamOS preinstalled tools gamescope mangohud gamemode disk space limited storage Flatpak redundancy`
6. `Flatpak host fallback bundled tools dual path complexity configuration detection pattern`
7. `Flatpak bundling security patches delayed upstream updates small maintainer team vulnerability window`
8. `Linux gaming Flatpak immutable distro first time setup no wine proton user experience onboarding wizard`
9. `Flatpak bundling ecosystem fragmentation Linux gaming community effect packaging duplicate tools`
10. `Heroic Games Launcher Bottles Lutris Flatpak bundling approach tools MangoHud GameMode gamescope strategy`
11. `Flatpak "uncanny valley" partial functionality some tools work others don't user confusion support burden`
12. `winetricks license LGPL redistribution bundling implications Flatpak 2025`
13. `Steam Deck storage 64GB model available space SteamOS system partition Flatpak overhead 2025`

## Sources

- [Flatpak Dependencies Documentation](https://docs.flatpak.org/en/latest/dependencies.html)
- [Flathub Requirements](https://docs.flathub.org/docs/for-app-authors/requirements)
- [Flathub Maintenance Documentation](https://docs.flathub.org/docs/for-app-authors/maintenance)
- [Flatpak Startup Overhead — Issue #2275](https://github.com/flatpak/flatpak/issues/2275)
- [Seccomp Filter Performance — Issue #4187](https://github.com/flatpak/flatpak/issues/4187)
- [Heroic MangoHud/Gamescope Runtime Matching — Issue #4791](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/4791)
- [Heroic Gamescope Wiki](<https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/How-To:-Other-Tools-and-Wrappers-(gamescope)>)
- [Heroic Steam Deck Wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Steam-Deck)
- [Heroic Missing Binaries Message — Issue #4317](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/4317)
- [Lapce Flatpak Fallback — Issue #1133](https://github.com/lapce/lapce/issues/1133)
- [Steam Deck FAQ — Steamworks](https://partner.steamgames.com/doc/steamdeck/faq)
- [Steam Deck 64GB Storage Discussion](https://steamcommunity.com/app/1675200/discussions/0/3385030647948351716/)
- [Steam Deck Storage Overload Warning](https://steamcommunity.com/app/1675200/discussions/0/3274688652643211345/)
- [SteamOS Install Size — PC Games N](https://www.pcgamesn.com/steam-deck/os-install-size)
- [When Flatpak's Sandbox Cracks — Linux Journal](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal)
- [Flatpak Security Nightmare — flatkill.org](https://flatkill.org/)
- [Snap vs Flatpak Comparison](https://machaddr.substack.com/p/snap-or-flatpak-on-linux-why-you)
- [Linux App Problem — XDA Developers](https://www.xda-developers.com/linuxs-app-problem-app-stores-refuse-merge/)
- [Steam Frame and Flatpak — It's FOSS](https://itsfoss.gitlab.io/post/steam-frame-machine-flatpaks-desktop-linux/)
- [Wine License Discussion — WineHQ Forums](https://forum.winehq.org/viewtopic.php?t=10117)
- [flatpak-builder License Issue #34](https://github.com/flatpak/flatpak-builder/issues/34)
- [winesapOS — GitHub](https://github.com/winesapOS/winesapOS)
- [Coding Horror — Uncanny Valley of UI](https://blog.codinghorror.com/avoiding-the-uncanny-valley-of-user-interface/)
- [Bazzite / Immutable Distro Gaming — Lemmy](https://lemmy.world/post/137705)
- [Firefox Flatpak vs Snap — Ctrl Blog](https://www.ctrl.blog/entry/firefox-linux-flatpak-snap.html)

## Uncertainties and Gaps

1. **No `flatpak-spawn --host` latency benchmarks exist.** The ~50-150ms estimate is extrapolated from related measurements, not directly measured. CrossHook should benchmark this.
2. **No data on CrossHook's actual user distribution across platforms.** The Steam Deck redundancy argument depends on what fraction of users are on Steam Deck.
3. **Heroic's support ticket data is not public.** The claim that partial bundling increases support load is inferred from GitHub issues, not quantified.
4. **No direct comparison of "setup wizard" vs "bundling" user success rates.** This would require A/B testing that hasn't been done in the Linux gaming space.
5. **Ecosystem convergence direction is uncertain.** Valve's Flatpak endorsement is strong, but whether this leads to better host-tool accessibility for sandboxed apps is speculative.
