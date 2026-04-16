# Evidence Quality Assessment: Flatpak Tool Bundling Research

**Perspective**: Evidence Weigher / Crucible
**Date**: 2026-04-15
**Scope**: Assess evidence quality, reliability, and CrossHook-specific relevance across all 8 Phase 1 research files
**Input files**: `01-historical.md`, `02-contrarian.md`, `03-analogical.md`, `04-systems.md`, `05-investigative.md`, `06-archaeological.md`, `07-futurist.md`, `08-negative-space.md`

---

## Executive Summary

The Phase 1 research corpus (~3,900 lines across 8 files) is **unusually strong for a decision-support document**. The majority of claims are grounded in primary sources (GitHub repositories, official documentation, CVE advisories, and direct source code analysis). However, several important weaknesses exist:

1. **Strongest evidence**: Claims derived from CrossHook's own source code (`04-systems`, `06-archaeological`) and from verifiable Flatpak manifests (`01-historical`, `05-investigative`) are rock-solid.
2. **Moderate evidence**: Cross-domain analogies (`03-analogical`) and ecosystem trend projections (`07-futurist`) are well-sourced but involve interpretive leaps when applied to CrossHook.
3. **Weakest evidence**: Performance overhead numbers (`08-negative-space`), binary size projections (`02-contrarian`), and community sentiment claims (`05-investigative` Section 6) rely on extrapolation, single studies, or anecdotal forum posts.
4. **Circular reasoning detected**: Three files (`01`, `02`, `04`) independently conclude "bundling doesn't work" but cite overlapping evidence (the same Lutris #6144 and gamescope #6 issues appear in all three). This creates an illusion of independent corroboration.
5. **Systematic bias**: The research corpus leans **anti-bundling** (6 of 8 files argue against or identify problems with bundling; only `07-futurist` makes a sustained pro-bundling argument). This may reflect genuine evidence weight or researcher selection bias.

**Overall corpus confidence**: **Medium-High** -- strong on factual claims, weaker on projections and CrossHook-specific applicability.

---

## Methodology

Each major claim is assessed on five dimensions:

| Dimension               | Scale                                                      | Description                                                                                                 |
| ----------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| **Source quality**      | Primary / Secondary / Hearsay                              | Is the evidence from the original source, a report about it, or an unverified claim?                        |
| **Freshness**           | Current (<1y) / Recent (1-2y) / Dated (2-4y) / Stale (>4y) | When was the evidence produced?                                                                             |
| **Verification**        | Verified / Plausible / Assumed                             | Has the claim been independently confirmed, is it logically sound but unconfirmed, or is it taken on faith? |
| **CrossHook relevance** | Direct / Analogical / Tangential                           | Does this apply specifically to CrossHook, to a similar app, or only loosely?                               |
| **Confidence**          | High / Medium / Low                                        | Overall assessment                                                                                          |

---

## Scored Evidence Matrix

### Tier 1: High-Confidence Claims (Strong Evidence, Directly Relevant)

| #   | Claim                                                                            | Files          | Source Quality                                      | Freshness    | Verification | CH Relevance | Confidence | Notes                                                                       |
| --- | -------------------------------------------------------------------------------- | -------------- | --------------------------------------------------- | ------------ | ------------ | ------------ | ---------- | --------------------------------------------------------------------------- |
| 1   | CrossHook routes all host tool access through `platform.rs` abstraction          | 04, 06         | Primary (source code)                               | Current      | Verified     | Direct       | **High**   | Read directly from `platform.rs` lines 1-1434                               |
| 2   | `flatpak-spawn --host` is a complete sandbox escape by design                    | 04, 05         | Primary (Flatpak docs, CVE advisories)              | Current      | Verified     | Direct       | **High**   | Confirmed by CVE-2021-21261, Flatpak docs, and `platform.rs` implementation |
| 3   | Winetricks must match the host Wine/Proton version that owns the prefix          | 01, 02, 04     | Primary (winetricks source, Wine architecture docs) | Current      | Verified     | Direct       | **High**   | Wine prefix architecture is fundamental; version coupling is inherent       |
| 4   | GameMode works from Flatpak via `org.freedesktop.portal.GameMode`                | 01, 02, 04, 05 | Primary (XDG portal spec, GameMode 1.4 release)     | Current      | Verified     | Direct       | **High**   | Portal spec is authoritative; GameMode 1.4 changelog confirms               |
| 5   | Lutris declares `--talk-name=org.freedesktop.Flatpak` and is accepted on Flathub | 01, 05         | Primary (manifest on GitHub)                        | Current      | Verified     | Analogical   | **High**   | Directly verified from `flathub/net.lutris.Lutris` manifest                 |
| 6   | CachyOS kernel optimizations cannot be bundled                                   | 02, 04         | Primary (kernel architecture)                       | Current      | Verified     | Direct       | **High**   | Structural fact: schedulers, sysctl, and CPU flags are kernel-level         |
| 7   | Winepak (per-app Wine bundling) failed and was abandoned                         | 01             | Primary (GitHub issues #17, #143)                   | Dated (2018) | Verified     | Analogical   | **High**   | Clear historical record with documented failure modes                       |
| 8   | Games launched by CrossHook run on the host, not in the sandbox                  | 04, 06         | Primary (source code)                               | Current      | Verified     | Direct       | **High**   | `flatpak-spawn --host` means the game process is unsandboxed                |
| 9   | Gamescope Flatpak extension is acknowledged as a hack by maintainers             | 01, 05         | Primary (GitHub repo README)                        | Current      | Verified     | Analogical   | **High**   | Direct quote from `org.freedesktop.Platform.VulkanLayer.gamescope`          |
| 10  | Flatpak gamescope does NOT work with official Proton nested sandbox              | 01, 02, 05     | Primary (GitHub issue #6)                           | Recent       | Verified     | Analogical   | **High**   | Confirmed by multiple independent reports                                   |
| 11  | CrossHook already correctly passes env vars via `--env=K=V` args                 | 04, 06         | Primary (source code)                               | Current      | Verified     | Direct       | **High**   | `host_command_with_env()` implementation verified in `platform.rs`          |
| 12  | `org.freedesktop.Flatpak` permission faces Flathub review scrutiny               | 05             | Primary (Flathub linter docs)                       | Current      | Verified     | Direct       | **High**   | Direct from official Flathub documentation                                  |

### Tier 2: Medium-Confidence Claims (Good Evidence, Some Interpretation Required)

| #   | Claim                                                                         | Files      | Source Quality                             | Freshness | Verification | CH Relevance | Confidence      | Notes                                                                                                                                                                                                                                                                                         |
| --- | ----------------------------------------------------------------------------- | ---------- | ------------------------------------------ | --------- | ------------ | ------------ | --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 13  | Ecosystem converged on internal runner managers + extensions/portals          | 01         | Secondary (synthesis of multiple projects) | Current   | Plausible    | Analogical   | **Medium-High** | Pattern is clear across Lutris, Bottles, Heroic, but CrossHook's trainer-launcher role differs from game launchers                                                                                                                                                                            |
| 14  | Bottles is CrossHook's closest Flatpak analogue                               | 03, 05     | Secondary (comparative analysis)           | Current   | Plausible    | Analogical   | **Medium**      | Bottles bundles Wine inside sandbox and does NOT use `flatpak-spawn --host`; CrossHook delegates everything to host. The tool dependency overlap is real but the architectural model diverges significantly                                                                                   |
| 15  | ~42% of Flatpak apps override or misconfigure sandboxing                      | 02, 05     | Secondary (Linux Journal study)            | Recent    | Plausible    | Tangential   | **Medium**      | Single study; methodology not independently audited; percentage may have changed                                                                                                                                                                                                              |
| 16  | Binary size would explode to 800MB-1.5GB with full bundling                   | 02         | Primary (pacman -Si sizes)                 | Current   | Plausible    | Direct       | **Medium**      | Individual package sizes are verified, but transitive dependency calculation is estimated, not measured. Flatpak dedup would reduce actual disk impact                                                                                                                                        |
| 17  | Immutable distros strengthen the bundling argument                            | 07, 08     | Secondary (trend analysis)                 | Current   | Plausible    | Direct       | **Medium-High** | SteamOS immutability is documented fact; growth trend is real; but magnitude of impact on CrossHook's user base is unknown                                                                                                                                                                    |
| 18  | Partial bundling creates worse UX than full commitment to either approach     | 08         | Secondary (Heroic issues, UI theory)       | Current   | Plausible    | Analogical   | **Medium**      | Heroic experience (#4791) supports this; "uncanny valley" concept is borrowed from UI theory, not empirically tested for Flatpak tool bundling                                                                                                                                                |
| 19  | MangoHud must be on the host because games run on the host                    | 04         | Primary (architecture analysis)            | Current   | Verified     | Direct       | **Medium-High** | Logic is sound for `flatpak-spawn --host` launched games; however, the VulkanLayer extension IS designed for exactly this purpose within Flatpak's own launch path. The claim is correct for CrossHook's specific architecture but overstates the general case                                |
| 20  | umu-launcher Flatpak packaging is "extremely difficult"                       | 01, 04, 05 | Primary (GitHub issue #430)                | Recent    | Verified     | Direct       | **Medium-High** | Issue #430 documents real problems; "extremely difficult" may overstate -- Heroic and Lutris Flatpaks both include umu support, just with workarounds                                                                                                                                         |
| 21  | Background portal could silently kill CrossHook-launched games                | 05         | Primary (portal docs, issue #1104)         | Current   | Plausible    | Direct       | **Medium**      | Portal docs confirm the mechanism, but CrossHook launches via `flatpak-spawn --host` -- the game process is on the HOST, not in the sandbox. The portal kills sandbox processes, not host processes spawned via `flatpak-spawn`. This claim may be **incorrect** for CrossHook's architecture |
| 22  | CVE-2026-34078 will make Flathub reviewers more cautious                      | 05         | Primary (CVE advisory)                     | Current   | Assumed      | Direct       | **Medium**      | The CVE is real; the behavioral prediction about reviewers is reasonable but unverifiable                                                                                                                                                                                                     |
| 23  | Wine Wayland driver will reach X11 parity by 2027-2028                        | 07         | Secondary (release timeline extrapolation) | Current   | Assumed      | Tangential   | **Medium**      | Trajectory is clear but timeline is speculative; Proton adoption lags upstream Wine                                                                                                                                                                                                           |
| 24  | "Detect + Prompt + Install" is the universal pattern across sandbox platforms | 03         | Secondary (cross-domain synthesis)         | Current   | Plausible    | Analogical   | **Medium-High** | Pattern is well-documented across macOS, Android, Bottles, Podman Desktop. Applicability to CrossHook is high but implementation complexity is underexplored                                                                                                                                  |

### Tier 3: Low-Confidence Claims (Weak Evidence, Speculative, or Wrongly Scoped)

| #   | Claim                                                                        | Files  | Source Quality                                           | Freshness                     | Verification | CH Relevance | Confidence     | Notes                                                                                                                                                                                                                                    |
| --- | ---------------------------------------------------------------------------- | ------ | -------------------------------------------------------- | ----------------------------- | ------------ | ------------ | -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 25  | `flatpak-spawn --host` adds ~50-150ms per invocation                         | 08     | Hearsay (extrapolated from unrelated Flatpak benchmarks) | Dated (issue #2275 from 2019) | Assumed      | Direct       | **Low**        | No direct benchmark exists. The 140ms figure is for Flatpak app startup (host→sandbox), not `flatpak-spawn --host` (sandbox→host). The D-Bus roundtrip mechanism differs. This number should not be cited without measurement            |
| 26  | Seccomp overhead of 3-19% for gaming workloads                               | 08     | Primary (issue #4187 benchmarks)                         | Dated (2021)                  | Verified     | Tangential   | **Low-Medium** | Benchmarks are real but measured for games running INSIDE a Flatpak sandbox. CrossHook launches games on the HOST -- seccomp overhead is irrelevant for the game process. Only relevant if tools were bundled AND ran inside the sandbox |
| 27  | systemd-appd will enable nested sandboxing for CrossHook trainers            | 07     | Secondary (blog post, no shipped code)                   | Current                       | Assumed      | Tangential   | **Low**        | No code exists. Timeline is 2+ years. Sebastian Wick himself cautioned about limitations. Speculative                                                                                                                                    |
| 28  | OGC will standardize gaming tool packaging                                   | 07     | Secondary (news articles)                                | Current                       | Assumed      | Tangential   | **Low**        | OGC is 3 months old. CachyOS (CrossHook's most popular distro) declined to join. Effectiveness is unproven                                                                                                                               |
| 29  | GPU virtualization (Venus/VirtIO-GPU) could make gamescope bundling feasible | 07     | Hearsay (exploratory blog post)                          | Current                       | Assumed      | Tangential   | **Low**        | Author acknowledges "a bunch of issues and unknowns." Performance overhead likely too high for gaming                                                                                                                                    |
| 30  | Maintenance cliff: CrossHook can't sustain patching 7 bundled tools          | 02, 08 | Secondary (ecosystem pattern)                            | Current                       | Plausible    | Direct       | **Medium-Low** | The pattern is real for the ecosystem, but the argument assumes CrossHook WOULD bundle all 7 tools. No one proposes this. The actual proposal would be 0-2 tools                                                                         |
| 31  | Flathub apps lag months behind distro security patches                       | 02, 08 | Secondary (flatkill.org, Linux Journal)                  | Dated                         | Plausible    | Tangential   | **Low-Medium** | flatkill.org is an advocacy site, not a neutral source. The Linux Journal article is a single study. The pattern exists but the severity claim is cherry-picked                                                                          |
| 32  | Game Status Portal will emerge                                               | 05     | Hearsay (discussion thread #1222)                        | Current                       | Assumed      | Tangential   | **Low**        | Open discussion with no committed development path                                                                                                                                                                                       |

---

## Claim-by-Claim Deep Analysis

### Theme A: CrossHook's Architecture Forces Host Delegation

**Claims**: #1, #2, #8, #11

**Evidence strength**: **Very High**

These claims are derived directly from reading CrossHook's source code (`platform.rs`, `script_runner.rs`, `runtime_helpers.rs`, `watchdog.rs`). Files `04-systems` and `06-archaeological` performed the most rigorous analysis here, with line-number references and function-level documentation.

**Cross-validation**: The Flatpak manifest (`packaging/flatpak/dev.crosshook.CrossHook.yml`) confirms the `--talk-name=org.freedesktop.Flatpak` permission and the `flatpak-spawn --host` dependency. Code and manifest are consistent.

**What would flip this**: A fundamental architectural change where CrossHook moves to bundling Wine/Proton internally (like Bottles does) instead of delegating to the host. This would require rewriting the entire launch chain -- years of work.

**Assessment**: These are the most reliable claims in the entire corpus. They are facts about the current codebase, not predictions or analogies.

---

### Theme B: Tool-Specific Bundleability

**Claims**: #3, #4, #6, #9, #10, #19, #20

**Evidence strength**: **High for most; Medium for MangoHud and umu-launcher**

The unbundleability of CachyOS kernel optimizations (#6) is a structural fact. The tight coupling between winetricks/winecfg and the host Wine version (#3) is well-documented with specific issue numbers (Winetricks #2218, #2084, #1442). The GameMode portal (#4) is documented in the official XDG spec.

**MangoHud nuance** (#19): The claim that "MangoHud must be on the host because games run on the host" is architecturally correct for CrossHook's `flatpak-spawn --host` model. However, the VulkanLayer extension mechanism (`org.freedesktop.Platform.VulkanLayer.MangoHud`) is designed to inject into Flatpak-launched processes. The research correctly identifies that this extension would apply to CrossHook's own window (irrelevant) rather than host-launched games, but this nuance could be clearer.

**umu-launcher** (#20): Issue #430 documents real packaging difficulties, but the claim of "extremely difficult" should be qualified. Three major launchers (Lutris, Heroic, faugus-launcher) have working umu integration in their Flatpaks, albeit with workarounds. "Requires significant workarounds" is more accurate than "extremely difficult."

**What would flip this**:

- **Winetricks**: If CrossHook bundled its own Wine/Proton (like Bottles), a bundled winetricks matching that Wine version would work. But this conflicts with CrossHook's architecture.
- **MangoHud**: If CrossHook moved to an architecture where games launch inside the sandbox (not via `flatpak-spawn --host`), the VulkanLayer extension would become the correct path.
- **Gamescope**: A properly designed Flatpak extension point for compositors (not the VulkanLayer hack) would change the calculus.

---

### Theme C: Ecosystem Precedents (Lutris, Bottles, Heroic, Winepak)

**Claims**: #5, #7, #13, #14

**Evidence strength**: **High for individual facts; Medium for pattern claims**

The Lutris Flathub precedent (#5) is verified from the actual manifest file. Winepak's failure (#7) is documented in its own issues. These are solid.

The "ecosystem converged" pattern claim (#13) is a synthesis across multiple projects. Each individual data point is verified, but the conclusion involves an interpretive leap: Lutris, Bottles, and Heroic are all _game launchers_ that need to run Wine themselves. CrossHook is a _trainer launcher_ that only orchestrates -- it never runs Wine directly. The convergence pattern is real but its applicability to CrossHook is weaker than the research implies.

**Bottles as closest analogue** (#14): This is stated in `03-analogical` and echoed in `05-investigative`, but the comparison has a significant flaw. Bottles bundles Wine inside its sandbox and does NOT use `flatpak-spawn --host`. CrossHook does the opposite -- it bundles nothing and delegates everything to the host. The tool dependency overlap (MangoHud, gamescope, winetricks) creates surface similarity, but the execution model is fundamentally different. **Podman Desktop** (thin GUI + host daemon delegation) may actually be a better architectural analogue, as `03-analogical` itself notes but does not emphasize.

**What would flip this**: If a new gaming launcher emerged that used `flatpak-spawn --host` for everything (like CrossHook) AND successfully bundled tools, it would invalidate the "precedent shows bundling inside sandbox" conclusion. Currently no such example exists.

---

### Theme D: Flathub Policy and Acceptance

**Claims**: #5, #12, #15, #22

**Evidence strength**: **High for policy text; Medium for behavioral predictions**

Flathub's official documentation (#12) clearly states that `--talk-name=org.freedesktop.Flatpak` is restricted and requires justification. This is primary source. The Lutris precedent (#5) demonstrates that gaming launchers CAN pass review with this permission.

The 42% sandbox misconfiguration statistic (#15) appears in both `02-contrarian` and `05-investigative`, creating an impression of independent corroboration. However, both cite the same Linux Journal article and flatkill.org. This is a **single source appearing as two**. The methodology of the study is not independently auditable, and flatkill.org is an advocacy site with an agenda.

The prediction that CVE-2026-34078 will make reviewers more cautious (#22) is reasonable but unfalsifiable until CrossHook actually submits to Flathub.

**What would flip this**:

- If Flathub implemented fine-grained `flatpak-spawn` command filtering (issue #5538), CrossHook could declare specific allowed commands instead of blanket host access. This would reduce review friction significantly.
- If Flathub tightened policy to reject `org.freedesktop.Flatpak` entirely for new apps (currently not proposed), CrossHook would need a fundamentally different approach.

---

### Theme E: Immutable Distros and the "Can't Install Host Tools" Problem

**Claims**: #17, #21 (Background portal), #25 (flatpak-spawn latency)

**Evidence strength**: **High for the trend; Medium for CrossHook-specific impact; Low for quantitative claims**

The growth of immutable distros (Bazzite, Fedora Atomic, SteamOS) is well-documented with multiple independent sources. SteamOS's read-only rootfs is a fact. The argument that these users cannot easily install host tools is structurally sound.

**However**, the research underweights a crucial counterpoint: Bazzite, the most popular gaming immutable distro, **pre-installs** gamescope, MangoHud, Steam, and gaming optimizations. SteamOS does the same. The "bare immutable distro" scenario (#21 in `08-negative-space`) is less common than implied because gaming-focused immutable distros ship with gaming tools pre-installed. The users most likely to need CrossHook are the ones most likely to already have the tools.

The exception is **Fedora Atomic (Silverblue/Kinoite)**, which is a general-purpose immutable distro without pre-installed gaming tools. This is a real gap but represents a smaller portion of the gaming audience.

**Background portal risk** (#21): `05-investigative` warns that `org.freedesktop.portal.Background` could silently kill CrossHook-launched games. This claim is **likely incorrect for CrossHook's architecture**. Games are launched via `flatpak-spawn --host`, creating HOST processes. The Background portal monitors SANDBOX processes. The game PID lives in the host PID namespace, not the sandbox. The portal would affect CrossHook's own Tauri process, not the games. This is a meaningful error in the research that could lead to unnecessary engineering work.

**What would flip this**:

- If Fedora Atomic became the dominant gaming distro (currently Bazzite dominates), the "bare immutable" problem would be much larger.
- If SteamOS removed pre-installed gaming tools in a future version (extremely unlikely given Valve's gaming focus).

---

### Theme F: Future Projections

**Claims**: #23, #27, #28, #29, #32

**Evidence strength**: **Low to Medium (inherently speculative)**

`07-futurist` is transparent about its speculative nature, assigning probability ranges to scenarios. This is intellectually honest. However:

- **systemd-appd** (#27): No shipped code exists. The projection assigns 2027-2028 timeline but acknowledges "no firm timeline." This is an aspiration, not evidence.
- **OGC standardization** (#28): The OGC is 3 months old and CachyOS declined. Betting on OGC standardization for CrossHook's packaging decisions would be premature.
- **Wine Wayland parity** (#23): The trajectory is clear but timelines are uncertain. GNOME 50 dropping X11 (a verified fact) creates forcing function pressure, but Wine/Proton adoption always lags upstream.

The scenario matrix in `07-futurist` Section 11.1 is useful as a thinking tool but the probability assignments (e.g., "Immutable distros dominate gaming: 70%") are essentially the author's subjective estimates, not data-driven calculations.

**What would flip this**: Any of these projections could be rendered moot by a single Valve decision (e.g., Valve endorsing a specific Flatpak model for third-party launchers).

---

### Theme G: Performance and Size

**Claims**: #16, #25, #26

**Evidence strength**: **Low for performance; Medium for size**

**Binary size** (#16): Individual package sizes from `pacman -Si` are verified. The 800MB-1.5GB estimate for full bundling with transitive dependencies is plausible but not measured. Flatpak's OSTree deduplication and shared runtime model would significantly reduce actual disk impact, and `02-contrarian` acknowledges this but then dismisses it as irrelevant for gaming users -- an assumption without data.

**flatpak-spawn latency** (#25): The ~50-150ms estimate is extrapolated from `flatpak/flatpak#2275`, which measures Flatpak app startup overhead (host→sandbox). `flatpak-spawn --host` goes the opposite direction (sandbox→host) via D-Bus. No direct benchmark exists. **This number should not be used in decision-making without measurement.**

**Seccomp overhead** (#26): The 3-19% benchmarks from `flatpak/flatpak#4187` are real measurements but from 2021 and measure games running INSIDE the sandbox. CrossHook launches games on the HOST. For CrossHook's architecture, seccomp overhead on the game process is zero. This claim is **inapplicable to CrossHook** as cited but would become relevant only if tools were bundled and ran inside the sandbox.

**What would flip this**: Direct benchmarks of `flatpak-spawn --host` latency and CrossHook's actual launch sequence overhead. A 500ms overhead per launch would be noticeable; a 50ms overhead would be negligible.

---

## Circular Reasoning and Cross-Citation Analysis

### Pattern 1: The Lutris Winetricks Bug Echo Chamber

The failure of winetricks in Lutris Flatpak 0.5.19 (GitHub #6144) is cited in:

- `01-historical.md` (Section 1, "What failed")
- `02-contrarian.md` (Section 4, winetricks paradox)
- `04-systems.md` (Section 3.1, "Known Flatpak Issue")
- `05-investigative.md` (Section 3.4, winetricks status)

Four files independently citing the same single issue creates an illusion of four independent data points. In reality, this is **one data point** (a specific library conflict in a specific Lutris version) being used to support the general claim "winetricks doesn't work in Flatpak." The claim may be correct, but the evidence base is thinner than it appears.

**Mitigation**: The underlying argument (Wine version coupling) is structurally sound even without this specific bug. The Lutris bug is illustrative, not foundational.

### Pattern 2: The Gamescope #6 Amplification

The gamescope Flatpak extension's incompatibility with Proton's nested sandbox (issue #6 on `flathub/org.freedesktop.Platform.VulkanLayer.gamescope`) is cited in:

- `01-historical.md` (Section 9)
- `02-contrarian.md` (Section 6, "Documented Flatpak Gamescope Failures")
- `04-systems.md` (Section 3.4)
- `05-investigative.md` (Section 3.2)

Same pattern: one issue, four citations, inflated apparent evidence weight. However, the issue is confirmed by the extension maintainers themselves, so the claim is reliable despite the amplification.

### Pattern 3: The 42% Sandbox Stat

The "~42% of Flatpak apps override or misconfigure sandboxing" statistic appears in:

- `02-contrarian.md` (Section 9)
- `05-investigative.md` (Section 1.3)

Both cite the same Linux Journal article and flatkill.org. This is a **single source with a known advocacy bias** presented as corroborated finding. The statistic may be directionally correct but should carry a "single unaudited source" warning.

### Pattern 4: Self-Reinforcing Anti-Bundling Conclusions

Files `01`, `02`, `03`, `04`, and `06` all conclude against bundling. Each cites the others' evidence indirectly (through the same primary sources). The research design assigns different "perspectives" to different files, but the underlying evidence pool is shared. The convergence of conclusions is partly genuine (the evidence genuinely points against bundling for most tools) and partly structural (the same issues keep appearing because they're the most-documented problems).

Only `07-futurist` makes a sustained argument that the trend might favor bundling (the immutable distro argument), and `08-negative-space` raises it as a blind spot. This 6:1:1 ratio may reflect evidence reality or may reflect researcher priors.

---

## Unfounded Assumptions

### Assumption 1: "CrossHook's Users Can Install Host Tools"

Multiple files assume users can run `pacman -S gamescope` or equivalent. This assumption is:

- **Valid** for Arch, Fedora Workstation, Ubuntu users
- **Partially valid** for Bazzite/Fedora Atomic users (requires `rpm-ostree`, reboot)
- **Invalid** for SteamOS users (changes don't survive updates)

**No file measures the actual distribution of CrossHook users across these platforms.** The user-base composition is the single most important unknown for the bundling decision, and no file addresses it empirically.

### Assumption 2: "Bottles Is the Closest Analogue"

Stated in `03-analogical` (Section 6) and reinforced in `05-investigative`. But:

- Bottles bundles Wine inside the sandbox. CrossHook does not.
- Bottles does NOT use `flatpak-spawn --host`. CrossHook does.
- Bottles' Flatpak-only distribution model has no native fallback. CrossHook has AppImage.

The tool overlap (MangoHud, gamescope, winetricks) creates surface similarity, but the execution model differs fundamentally. **VS Code Flatpak or Podman Desktop** may be more architecturally relevant analogues (both use host delegation), though their domain differs.

### Assumption 3: "Bundling Means Bundling All 7 Tools"

`02-contrarian` constructs the worst case (bundling all 7 tools = 800MB-1.5GB, 7 update cycles). `08-negative-space` extends this to a testing matrix of 240+ scenarios. But the actual decision space includes:

- Bundle 0 tools (current approach)
- Bundle 1-2 carefully selected tools
- Use Flatpak extensions for 2-3 tools
- Build some features natively (Proton download manager)

The "all or nothing" framing appears in `08-negative-space` (Section 2, "bundle everything or nothing") as a recommendation, but the research doesn't rigorously compare selective bundling strategies.

### Assumption 4: "Background Portal Will Kill CrossHook's Games"

`05-investigative` (Section 4.2) warns about `org.freedesktop.portal.Background` silently killing game processes. This is **likely wrong for CrossHook**. Games are launched via `flatpak-spawn --host`, creating host-side processes. The Background portal monitors sandbox processes. The CrossHook Tauri window (sandbox process) might be targeted, but the game/trainer processes (host processes) would be unaffected. This assumption could lead to unnecessary engineering work on Background portal integration.

### Assumption 5: "Seccomp Overhead Applies to CrossHook's Games"

`08-negative-space` (Section 5) cites 3-19% seccomp overhead for games. These benchmarks measure games running INSIDE a Flatpak sandbox. CrossHook's games run on the HOST via `flatpak-spawn --host`. Seccomp overhead is zero for host-spawned processes. This is incorrectly scoped.

---

## Evidence Gaps

### Gap 1: CrossHook's Actual User Base Composition (CRITICAL)

No file attempts to measure or estimate:

- What percentage of CrossHook users are on Steam Deck vs. desktop
- What percentage use immutable distros vs. traditional
- What percentage already have gaming tools installed vs. bare systems
- What the install success rate is for the current AppImage distribution

**This is the most important unknown.** The bundling decision depends fundamentally on who the users are, and no data exists.

### Gap 2: `flatpak-spawn --host` Latency (IMPORTANT)

No direct benchmark exists for `flatpak-spawn --host` invocation latency. `08-negative-space` estimates 50-150ms but extrapolates from unrelated measurements. For CrossHook's launch sequence (3-4 sequential `flatpak-spawn` calls), the cumulative overhead matters. This could be measured in minutes with a simple benchmark script.

### Gap 3: Disk Space Impact with Flatpak Deduplication

`02-contrarian` estimates 800MB-1.5GB for full bundling but acknowledges Flatpak's OSTree deduplication without quantifying it. The actual incremental disk cost depends on which runtime libraries are already shared. No file measures this.

### Gap 4: Heroic/Lutris Flatpak Support Ticket Data

Multiple files cite Heroic's Flatpak issues as evidence of bundling-related support burden. But no file quantifies:

- What fraction of Heroic's GitHub issues are Flatpak-specific
- How many involve tool version confusion
- Whether Heroic's support load increased or decreased after specific bundling decisions

This data is partially available from GitHub issue trackers but was not collected.

### Gap 5: Protontricks as an Alternative to Winetricks

`06-archaeological` documents CrossHook's winetricks/protontricks detection chain (winetricks > protontricks). `05-investigative` notes that Protontricks (`com.github.Matoking.protontricks`) is available on Flathub as a standalone Flatpak. No file explores whether Protontricks-as-Flatpak could serve CrossHook's prefix dependency management needs without bundling winetricks.

### Gap 6: Flatpak Extension Auto-Install Feasibility

Multiple files note that Flatpak extensions are not auto-installed. `03-analogical` identifies this as a UX problem. But no file investigates whether CrossHook's Flatpak manifest could declare recommended extensions that `flatpak install` would resolve, or whether programmatic extension installation is possible from within a Flatpak app.

---

## What Would Flip Major Conclusions

| Current Conclusion                                         | What Would Flip It                                                                                                                 | Likelihood                           |
| ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| "Don't bundle any tools"                                   | Evidence that >50% of CrossHook Flatpak users have a bare system without gaming tools                                              | Unknown (no data)                    |
| "Use flatpak-spawn --host for everything"                  | Flathub rejecting CrossHook's `org.freedesktop.Flatpak` permission request                                                         | Low-Medium (Lutris precedent exists) |
| "GameMode works via portal, no bundling needed"            | Discovery that the GameMode PID registration bug (#1270) affects CrossHook's use case                                              | Medium (bug status unclear)          |
| "Gamescope can't be bundled"                               | A properly designed compositor extension point in Flatpak (not the VulkanLayer hack)                                               | Low (no proposal exists)             |
| "Bundling is unsustainable for a small team"               | CrossHook growing to have 5+ active maintainers with dedicated packaging resources                                                 | Low (currently small project)        |
| "Immutable distro trend favors host-delegation model"      | SteamOS or Bazzite removing pre-installed gaming tools                                                                             | Very Low                             |
| "Proton version management is the only viable integration" | A well-packaged umu-launcher Flatpak that handles all Proton orchestration, eliminating CrossHook's need to manage Proton versions | Medium (umu is maturing rapidly)     |

---

## Overall Reliability Assessment

### What the Research Gets Right

1. **Architecture analysis is excellent.** Files `04-systems` and `06-archaeological` provide the strongest evidence base because they read and document actual source code. Any decision should weight these files heavily.

2. **Tool-specific bundleability analysis is sound.** The conclusion that most tools cannot be meaningfully bundled in CrossHook's `flatpak-spawn --host` architecture is well-supported by multiple independent arguments.

3. **Historical precedents are well-documented.** The Lutris, Bottles, Heroic, and Winepak case studies in `01-historical` are grounded in primary sources.

4. **The negative space analysis raises genuine blind spots.** The first-run experience gap, testing matrix explosion, and sustainability framing in `08-negative-space` are valuable contributions that other files miss.

### What the Research Gets Wrong or Overstates

1. **Performance claims are unreliable.** The `flatpak-spawn --host` latency estimate and seccomp overhead numbers should not be cited without direct measurement.

2. **The Background portal warning is likely misapplied.** Games launch on the host; the portal monitors sandbox processes.

3. **Bottles-as-analogue is misleading.** The execution model divergence (Bottles bundles Wine; CrossHook delegates to host) undermines the comparison.

4. **Anti-bundling bias in the corpus.** Six of eight files argue against or identify problems with bundling. Only `07-futurist` presents a sustained pro-bundling case. The research may underweight the real usability cost of "install 7 things on your host" for non-technical users.

5. **Cross-citation creates inflated apparent evidence.** The same Lutris #6144, gamescope #6, and 42% sandbox stats appear in 3-4 files each, making the evidence base look broader than it is.

### Reliability Ranking of Source Files

| File                | Role                 | Reliability     | Reasoning                                                                                          |
| ------------------- | -------------------- | --------------- | -------------------------------------------------------------------------------------------------- |
| `06-archaeological` | Code dig             | **Highest**     | Primary source analysis of actual CrossHook source code                                            |
| `04-systems`        | Dependency mapping   | **High**        | Combines source code analysis with external documentation                                          |
| `01-historical`     | Precedent survey     | **High**        | Well-sourced from GitHub repos and official docs                                                   |
| `05-investigative`  | Current ecosystem    | **High**        | Primary sources (manifests, docs, CVEs); one error (Background portal)                             |
| `02-contrarian`     | Devil's advocate     | **Medium-High** | Sound arguments but some overstatement (size estimates, maintenance burden assumes all-or-nothing) |
| `03-analogical`     | Cross-domain lessons | **Medium**      | Good pattern extraction but Bottles analogue is misleading                                         |
| `08-negative-space` | Blind spots          | **Medium**      | Raises important questions but performance claims are unreliable                                   |
| `07-futurist`       | Projections          | **Medium-Low**  | Inherently speculative; transparent about uncertainty but timelines are guesses                    |

---

## Sources Referenced in This Assessment

This document is a meta-analysis of the 8 Phase 1 research files. All sources cited are drawn from those files' own source lists. No additional external research was conducted for the evidence assessment itself. The assessment relies on:

1. Internal consistency checking across the 8 files
2. Cross-referencing claims against their cited primary sources
3. Evaluating whether cited sources actually support the claims made
4. Identifying where the same source appears in multiple files
5. Assessing whether CrossHook's specific architecture (vs. generic Flatpak advice) is properly accounted for
