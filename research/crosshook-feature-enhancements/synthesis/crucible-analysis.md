# Analysis of Competing Hypotheses: CrossHook's Optimal Strategic Direction

**Date**: 2026-03-19
**Method**: Analysis of Competing Hypotheses (ACH)
**Input**: Findings from 8 research personas (Historian, Contrarian, Analogist, Systems Thinker, Journalist, Archaeologist, Futurist, Negative Space Explorer)

---

## Executive Summary

Seven hypotheses about CrossHook's optimal strategic direction were evaluated against 28 pieces of evidence extracted from eight research personas. The ACH methodology -- which prioritizes disconfirming evidence over confirming evidence -- systematically eliminated two hypotheses (H5: Full Architectural Migration, H7: Status Quo Incrementalism) and significantly weakened two others (H3: UI/UX-First, H6: Steam Deck Exclusivity).

**Three hypotheses survived with the least disconfirming evidence:**

1. **H4: Community Platform Strategy** -- Build CrossHook into a platform around community-contributed profiles and a compatibility database. This hypothesis has the broadest support across personas (6 of 8 explicitly advocate elements of it) and the least disconfirming evidence. The strongest endorsement comes from the Systems Thinker's identification of a compatibility database as the "single highest-leverage feature" and the Analogist's extensive documentation of community repository patterns (Lutris, ProtonDB, ROM hacking) that prove the model works for this exact audience.

2. **H2: Multi-Tier Modification System** -- Implement a tiered approach from config patching through memory writes to DLL injection. This hypothesis is uniquely well-supported by the Archaeologist's and Historian's historical evidence that simpler techniques are more WINE-compatible, and the Contrarian's observation that CrossHook's current approach stacks multiple compatibility risks.

3. **H1: Technical Depth in Injection Reliability** -- Deepen injection method diversity and WINE-specific reliability. This hypothesis survives because the Systems Thinker identifies "technical depth, not features" as CrossHook's moat, but it is weakened by the Contrarian's argument that improving injection reliability faces diminishing returns under WINE's architectural constraints.

**The recommended synthesis**: Pursue H4 (Community Platform) as the primary strategy, with H2 (Multi-Tier Modification) as the technical foundation, and selective elements of H1 (injection method abstraction) as the engineering enabler. This combination addresses the largest adoption barriers (setup complexity, configuration discoverability) while reducing dependency on WINE's most fragile APIs.

---

## Step 1: Hypotheses

### H1: Technical Depth -- Perfect DLL Injection Reliability Under WINE

**Description**: CrossHook should focus engineering effort on making DLL injection maximally reliable under WINE/Proton. This means implementing multiple injection methods (manual mapping, thread hijacking, APC injection), building a WINE-specific compatibility layer, and creating a comprehensive testing framework that validates injection across Proton versions.

**Proponents**: Historian (injection technique lifecycle), Systems Thinker (technical depth as moat), Futurist (injection method abstraction)

**Implications**: Heavy R&D investment in low-level Win32/WINE internals; narrow feature set but deep reliability; engineering-intensive with uncertain payoff due to WINE's moving target nature.

### H2: Multi-Tier Modification System

**Description**: CrossHook should implement a hierarchy of game modification techniques ranked by WINE compatibility: Tier 0 (config file patching, 100% compatible), Tier 1 (direct memory read/write, high compatibility), Tier 2 (CreateRemoteThread + LoadLibrary, current approach), Tier 3 (advanced injection for edge cases). Users benefit from automatic fallback through tiers.

**Proponents**: Archaeologist (tiered injection table), Historian (simpler-is-better under WINE), Contrarian (alternative approaches: LD_PRELOAD, WINEDLLOVERRIDES, DLL proxy loading)

**Implications**: Broadens CrossHook's utility beyond DLL injection; reduces dependency on WINE's most fragile APIs; requires significant new capability development but each tier delivers standalone value.

### H3: UI/UX and Accessibility First

**Description**: CrossHook should prioritize making the existing functionality maximally accessible and usable: first-run setup wizard, controller-friendly Steam Deck UI, accessibility features (screen reader support, high contrast), in-app documentation, and reduced setup friction.

**Proponents**: Negative Space Explorer (setup cliff, accessibility void), Journalist (Steam Deck UI patterns, controller navigation), Analogist (Heroic's Big Picture mode)

**Implications**: Lower engineering risk; immediate user impact; does not address underlying technical limitations; may hit WinForms ceiling for UI sophistication under WINE.

### H4: Community Platform -- Profiles, Compatibility Database, Sharing

**Description**: CrossHook should transform from a standalone tool into a platform by building: structured profile manifests (JSON/TOML replacing flat files), a community-contributed compatibility database (ProtonDB-style for trainers), profile export/import/sharing, and in-app community browsing. Network effects become the competitive moat.

**Proponents**: Systems Thinker (highest-leverage intervention), Analogist (Lutris community scripts, VS Code extension model, ROM hacking distribution), Journalist (community-driven content as adoption driver), Negative Space Explorer (community sharing as critical absence), Historian (distribution problem is unsolved for Linux), Archaeologist (community-contributed game profiles essential for scale)

**Implications**: Transforms competitive dynamics from tool competition to platform competition; requires community infrastructure investment; value compounds over time; risk of insufficient critical mass.

### H5: Full Architectural Migration (Avalonia UI, Native Linux Mechanisms)

**Description**: CrossHook should undergo a fundamental architectural migration: replace WinForms with Avalonia UI for native Linux rendering, replace CreateRemoteThread injection with native Linux mechanisms (LD_PRELOAD, ptrace), and eliminate the WINE dependency for the tool itself (while games still run under Proton).

**Proponents**: Contrarian (WinForms is wrong for platform, native alternatives exist), Futurist (Avalonia as WinForms replacement, NativeAOT)

**Implications**: Highest engineering cost; longest time-to-value; highest risk (migration historically kills projects); potentially highest long-term payoff if completed successfully.

### H6: Steam Deck as Primary Platform

**Description**: CrossHook should narrow its focus exclusively to Steam Deck: optimize UI for 7-inch 1280x800 display, controller-only navigation, Game Mode integration, Gamescope overlay compatibility, and SteamOS-specific packaging (Flatpak).

**Proponents**: Journalist (Steam Deck as growth driver), Systems Thinker (Steam Deck is the growth engine), Futurist (Gamescope integration)

**Implications**: Maximum focus; clear target hardware; limits addressable market to Steam Deck owners; risks over-specialization if Steam Deck market shifts.

### H7: Status Quo -- Incremental Improvements Only

**Description**: CrossHook's current architecture and approach are adequate. Focus on bug fixes, minor UI polish, compatibility updates as Proton evolves, and incremental feature additions. No architectural changes needed.

**Proponents**: None explicitly; implicit in the Historian's observation that "switching frameworks historically kills projects" and the Archaeologist's "simplicity is a feature" principle.

**Implications**: Lowest risk; lowest investment; lowest potential upside; may result in slow irrelevance as Proton improves or competitors emerge.

---

## Step 2: Evidence Inventory

The following 28 pieces of evidence were extracted from persona findings, selected for their ability to discriminate between hypotheses.

| #   | Evidence                                                                                                                                                     | Source Persona              | Confidence  |
| --- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------- | ----------- |
| E1  | CreateRemoteThread under WINE is unreliable: incomplete implementation, address space differences, pressure-vessel isolation                                 | Contrarian                  | High        |
| E2  | WinForms under WINE has persistent rendering issues: GDI+ artifacts, timer reliability, DPI problems, file dialogs                                           | Contrarian                  | High        |
| E3  | Linux gaming market is ~2% of Steam (~3-4M users); trainer-seeking subset is far smaller (est. 15K-40K)                                                      | Contrarian                  | Medium      |
| E4  | Native Linux alternatives exist: LD_PRELOAD, WINEDLLOVERRIDES, ptrace, DLL proxy loading                                                                     | Contrarian, Historian       | High        |
| E5  | Anti-cheat proliferation makes injection techniques increasingly blocked even for single-player games                                                        | Contrarian, Systems Thinker | High        |
| E6  | Feature enhancement debt: each feature must work across WINE versions x Proton versions x game updates -- multiplicative maintenance burden                  | Contrarian, Systems Thinker | Medium      |
| E7  | DLL proxy loading (dinput8.dll, ASI loader) is historically more reliable under WINE than CreateRemoteThread                                                 | Historian, Archaeologist    | Medium      |
| E8  | CreateRemoteThread + LoadLibrary has survived 25+ years; proven durable for single-player games                                                              | Historian                   | Medium      |
| E9  | Every successful trainer platform solved a distribution/discovery problem, not just a technical one                                                          | Historian                   | Medium      |
| E10 | Trainer community has 30-year history of rejecting centralized, subscription-based tools (preference for local, offline, open tools)                         | Historian                   | Medium      |
| E11 | Running under WINE is an advantage for modification: kernel-mode anti-cheat does not function, user-space implementation bypasses protections                | Historian, Archaeologist    | Medium      |
| E12 | Switching UI frameworks historically kills projects (scope creep during migration)                                                                           | Historian                   | Medium      |
| E13 | CrossHook most closely resembles a DAW plugin host structurally; DAW patterns (sandbox validation, plugin scanning, load ordering) are directly transferable | Analogist                   | High        |
| E14 | Every successful plugin/module system uses structured manifests; flat-file profiles are the architectural bottleneck                                         | Analogist                   | High        |
| E15 | Community profiles/scripts are the proven growth flywheel for Linux gaming tools (Lutris: 6000+ community scripts)                                           | Analogist                   | High        |
| E16 | Compatibility database (ProtonDB model for trainers) is the single highest-leverage intervention -- creates network effects                                  | Systems Thinker             | High        |
| E17 | CrossHook's value will shift from "makes trainers work" to "makes trainers convenient" as Proton improves                                                    | Systems Thinker             | Medium      |
| E18 | Tool consolidation risk: Lutris or Bottles could absorb trainer-launch capabilities                                                                          | Systems Thinker             | Medium      |
| E19 | WeMod (100M+ claimed members) is market leader but has no Linux support and faces subscription fatigue                                                       | Journalist                  | High        |
| E20 | No trainer tool has prioritized Linux/Proton as first-class platform -- CrossHook occupies unique niche                                                      | Journalist                  | High        |
| E21 | Pattern scanning (AOB) is the single most impactful technique for robust, version-independent cheat definitions                                              | Archaeologist               | High        |
| E22 | Config/INI file patching is 100% WINE-compatible and many games still use text configs                                                                       | Archaeologist               | High        |
| E23 | NativeAOT compilation offers highest ROI near-term improvement: smaller binary, faster startup, better WINE compatibility                                    | Futurist                    | Medium-High |
| E24 | Avalonia UI is the strongest candidate for WinForms replacement but Win32 P/Invoke would not work natively -- requires split architecture                    | Futurist                    | High        |
| E25 | Accessibility is entirely absent from every game trainer tool; WinForms actually has better accessibility foundation than competitors                        | Negative Space              | High        |
| E26 | The 13+ step setup process for Steam Deck is the primary adoption barrier; 90%+ of potential users are filtered out before first use                         | Negative Space              | High        |
| E27 | Proton prefix isolation is the #1 technical pain point; no tool auto-manages prefix configuration for trainers                                               | Negative Space              | High        |
| E28 | No tool offers community sharing of trainer configurations; every user starts from zero repeating work others have done                                      | Negative Space              | High        |

---

## Step 3: Evidence vs Hypotheses Matrix

**Legend**: C = Consistent (evidence supports hypothesis), I = Inconsistent (evidence contradicts hypothesis), N = Neutral (evidence neither supports nor contradicts)

| Evidence                                    | H1: Technical Depth | H2: Multi-Tier | H3: UI/UX First | H4: Community Platform | H5: Full Migration | H6: Steam Deck Focus | H7: Status Quo |
| ------------------------------------------- | :-----------------: | :------------: | :-------------: | :--------------------: | :----------------: | :------------------: | :------------: |
| E1: CRT unreliable under WINE               |          C          |       C        |        N        |           N            |         C          |          N           |     **I**      |
| E2: WinForms rendering issues               |          N          |       N        |      **I**      |           N            |         C          |        **I**         |     **I**      |
| E3: Small market (15K-40K)                  |          N          |       N        |        N        |         **I**          |       **I**        |        **I**         |       C        |
| E4: Native Linux alternatives exist         |        **I**        |       C        |        N        |           N            |         C          |          N           |     **I**      |
| E5: Anti-cheat proliferation                |        **I**        |       C        |        N        |           N            |         N          |          N           |     **I**      |
| E6: Multiplicative maintenance burden       |        **I**        |       N        |        N        |           N            |       **I**        |          N           |       C        |
| E7: DLL proxy more reliable under WINE      |        **I**        |       C        |        N        |           N            |         N          |          N           |     **I**      |
| E8: CRT+LoadLibrary durable 25 years        |          C          |       C        |        N        |           N            |       **I**        |          N           |       C        |
| E9: Success = solving distribution          |        **I**        |       N        |        N        |           C            |         N          |          N           |     **I**      |
| E10: Community prefers local/open tools     |          N          |       N        |        N        |           C            |         N          |          N           |       C        |
| E11: WINE advantage for modification        |          C          |       C        |        N        |           N            |       **I**        |          N           |       C        |
| E12: Framework switches kill projects       |          N          |       N        |        N        |           N            |       **I**        |          N           |       C        |
| E13: DAW plugin host structural analog      |          C          |       C        |        N        |           C            |         N          |          N           |     **I**      |
| E14: Structured manifests needed            |          N          |       N        |        N        |           C            |         N          |          N           |     **I**      |
| E15: Community scripts proven flywheel      |          N          |       N        |        N        |           C            |         N          |          N           |     **I**      |
| E16: Compat DB highest leverage             |          N          |       N        |        N        |           C            |         N          |          N           |     **I**      |
| E17: Value shifts to convenience            |        **I**        |       C        |        C        |           C            |         N          |          N           |     **I**      |
| E18: Tool consolidation risk                |          C          |       N        |        N        |           C            |         N          |        **I**         |     **I**      |
| E19: WeMod has no Linux support             |          N          |       N        |        N        |           C            |         N          |          C           |       C        |
| E20: No tool targets Linux/Proton           |          C          |       C        |        C        |           C            |         N          |          C           |       C        |
| E21: AOB scanning most impactful            |          C          |       C        |        N        |           N            |         N          |          N           |     **I**      |
| E22: Config patching 100% WINE-compatible   |          N          |       C        |        N        |           N            |         N          |          N           |     **I**      |
| E23: NativeAOT highest ROI                  |          N          |       N        |        N        |           N            |         C          |          N           |     **I**      |
| E24: Avalonia requires split arch           |          N          |       N        |        N        |           N            |       **I**        |          N           |       N        |
| E25: Accessibility void; WinForms advantage |          N          |       N        |        C        |           N            |       **I**        |          N           |     **I**      |
| E26: 13-step setup cliff                    |          N          |       N        |        C        |           C            |         N          |          C           |     **I**      |
| E27: Prefix isolation pain point            |          N          |       C        |        N        |           C            |         C          |          C           |     **I**      |
| E28: No community config sharing            |          N          |       N        |        N        |           C            |         N          |          N           |     **I**      |

---

## Step 4: Critical Disconfirming Evidence

The power of ACH lies in disconfirming evidence. For each hypothesis, the evidence that most strongly argues against it:

### H1: Technical Depth -- Disconfirming Evidence

| Evidence                                              | Why It Disconfirms                                                                                                                                                                 |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E4: Native alternatives exist (LD_PRELOAD, proxy DLL) | Investing in perfecting Win32 injection under WINE is fighting the wrong battle when WINE-native mechanisms exist that bypass the problem entirely.                                |
| E5: Anti-cheat proliferation                          | Even perfected injection gets blocked by anti-cheat in a growing number of single-player games, creating diminishing returns for injection reliability investment.                 |
| E6: Multiplicative maintenance burden                 | Each new injection method must be tested across WINE versions x Proton versions x games. The testing matrix grows faster than engineering capacity.                                |
| E7: DLL proxy more reliable                           | Historical evidence shows a simpler, different approach (DLL proxy loading) outperforms the injection approach CrossHook is trying to perfect.                                     |
| E9: Success = solving distribution                    | The historical record shows that tools won through distribution/community, not through technical superiority of injection. Investing in injection depth attacks the wrong problem. |
| E17: Value shifts to convenience                      | As Proton improves, injection will "just work" -- meaning the investment in injection reliability has a declining window of relevance.                                             |

**Assessment**: H1 has 6 pieces of disconfirming evidence, the most of any surviving hypothesis. It remains viable only in a narrow interpretation: abstracting injection methods so that CrossHook can switch between them, rather than deeply perfecting any single method.

### H2: Multi-Tier Modification -- Disconfirming Evidence

| Evidence                                    | Why It Disconfirms |
| ------------------------------------------- | ------------------ |
| (None strongly contradicts this hypothesis) |                    |

**Assessment**: H2 has zero strong disconfirming evidence. Every persona that discussed technical approaches either explicitly advocated tiered modification (Archaeologist, Historian) or produced evidence consistent with it (Contrarian's native alternatives, Futurist's plugin architecture). This makes H2 the most robust hypothesis on a technical dimension but does not address the community/adoption dimension.

### H3: UI/UX First -- Disconfirming Evidence

| Evidence                                 | Why It Disconfirms                                                                                                                                                                                                                                             |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E2: WinForms rendering issues under WINE | WinForms imposes a hard ceiling on UI quality under WINE. Investing heavily in UI/UX within WinForms hits diminishing returns due to WINE rendering limitations (GDI+ artifacts, DPI issues, timer reliability). The very framework constrains the investment. |

**Assessment**: H3 has one strong disconfirming evidence but it is severe: the WinForms-under-WINE constraint means that UI investment has a hard ceiling. The Negative Space Explorer's setup wizard recommendation and the Journalist's controller navigation recommendation are achievable within WinForms, but a truly modern, accessible, controller-friendly UI would eventually require framework migration -- which is H5, and H5 has significant disconfirming evidence of its own. H3 is viable as a complement to another strategy but is insufficient as a primary strategy.

### H4: Community Platform -- Disconfirming Evidence

| Evidence                   | Why It Disconfirms                                                                                                                                                                                                                                                                           |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E3: Small market (15K-40K) | Community platforms require critical mass to generate network effects. A total addressable market of 15K-40K may be too small to reach the tipping point where community contributions become self-sustaining. Lutris succeeded with a larger market (all Linux gamers), not a niche subset. |

**Assessment**: H4 has only one piece of disconfirming evidence, but it is significant. The small market size creates a chicken-and-egg problem: the platform is most valuable when it has many contributors, but the small audience may never generate enough contributions. However, the Contrarian's market estimate (15K-40K) carries only "Low" confidence by the Contrarian's own assessment, and the Futurist predicts the Linux gaming market growing to 4-6% by 2027 (potentially doubling the base). The Journalist notes SteamOS expanding to third-party handhelds (Lenovo Legion Go S), further expanding the market. This disconfirming evidence is real but may be time-limited.

### H5: Full Architectural Migration -- Disconfirming Evidence

| Evidence                                  | Why It Disconfirms                                                                                                                                                                                                    |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E3: Small market (15K-40K)                | A major architectural rewrite is disproportionate to the market size. The engineering investment cannot be justified by the addressable user base.                                                                    |
| E6: Multiplicative maintenance burden     | Migration to a split architecture (native Linux UI + WINE engine subprocess) doubles the maintenance surface area rather than simplifying it.                                                                         |
| E8: CRT+LoadLibrary durable 25 years      | The current injection technique works and has proven historical durability. Replacing it with native Linux mechanisms solves a problem that may not need solving.                                                     |
| E11: WINE advantage for modification      | Running under WINE is an advantage, not a limitation, for game modification. Migrating away from WINE eliminates this advantage.                                                                                      |
| E12: Framework switches kill projects     | Historical evidence from multiple domains shows that major framework migrations are the leading cause of project death. The migration itself, not the destination framework, is the risk.                             |
| E24: Avalonia requires split architecture | Avalonia runs natively on Linux but Win32 P/Invoke (CrossHook's core) would not work natively. This forces a complex split architecture (native UI + WINE engine process) that is architecturally novel and unproven. |
| E25: WinForms accessibility advantage     | WinForms has better built-in accessibility (UI Automation / MSAA) than competing frameworks. Migrating away from WinForms loses this accessibility foundation.                                                        |

**Assessment**: H5 has 7 pieces of disconfirming evidence -- the most of any hypothesis. The Historian's observation that "switching frameworks historically kills projects" is particularly damning because it applies directly to CrossHook's situation: a small-team project with a niche audience cannot afford a multi-year migration. H5 is **eliminated** as a primary strategy. Elements of it (NativeAOT compilation, LibraryImport migration) are low-risk tactical improvements that can be adopted independently.

### H6: Steam Deck Exclusivity -- Disconfirming Evidence

| Evidence                      | Why It Disconfirms                                                                                                                                      |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E2: WinForms rendering issues | WinForms is hostile to Steam Deck's 7-inch touchscreen and controller input. Optimizing for Steam Deck within WinForms is fighting the framework.       |
| E3: Small market              | Narrowing to Steam Deck only further reduces an already small market.                                                                                   |
| E18: Tool consolidation risk  | A Steam Deck-focused tool is more likely to be absorbed by general-purpose launchers (Lutris, Bottles, Decky Loader) that already have Deck-native UIs. |

**Assessment**: H6 has 3 pieces of disconfirming evidence. The most damaging is E18: a Steam Deck-exclusive tool is exactly the kind of narrow scope that larger tools can easily absorb. The Systems Thinker's own observation that "CrossHook's moat is technical depth" contradicts a platform-specific UI focus. H6 is weakened but not eliminated -- Steam Deck should be a priority audience, not the exclusive audience.

### H7: Status Quo -- Disconfirming Evidence

| Evidence                               | Why It Disconfirms                                                                                                   |
| -------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| E1: CRT unreliable under WINE          | The current approach has known reliability problems that will not self-resolve.                                      |
| E2: WinForms rendering issues          | Current UI has WINE rendering problems that degrade user experience.                                                 |
| E4: Native alternatives exist          | Better approaches exist and are not being pursued.                                                                   |
| E5: Anti-cheat proliferation           | The environment is actively becoming more hostile to the current approach.                                           |
| E7: DLL proxy more reliable            | A superior technique is not being utilized.                                                                          |
| E9: Success = solving distribution     | The current approach does not address the distribution/discovery problem that historically determines tool survival. |
| E13: DAW plugin host patterns          | Proven architectural patterns are not being applied.                                                                 |
| E14: Structured manifests needed       | Current flat-file profiles are an architectural bottleneck.                                                          |
| E15: Community scripts proven flywheel | The proven growth mechanism is not being built.                                                                      |
| E16: Compat DB highest leverage        | The highest-leverage feature is not being developed.                                                                 |
| E17: Value shifts to convenience       | The current value proposition is time-limited; without evolution, CrossHook becomes irrelevant.                      |
| E18: Tool consolidation risk           | Stagnation invites absorption by larger tools.                                                                       |
| E21: AOB scanning most impactful       | High-impact capability is not being developed.                                                                       |
| E22: Config patching 100% compatible   | A zero-risk modification approach is not being offered.                                                              |
| E23: NativeAOT highest ROI             | The best ROI improvement is not being pursued.                                                                       |
| E25: Accessibility void                | A differentiating opportunity is being ignored.                                                                      |
| E26: 13-step setup cliff               | The primary adoption barrier is not being addressed.                                                                 |
| E27: Prefix isolation pain             | The #1 technical pain point is not being solved.                                                                     |
| E28: No community sharing              | The most conspicuous gap is not being filled.                                                                        |

**Assessment**: H7 has 19 pieces of disconfirming evidence -- overwhelmingly the most. Every persona produced evidence that contradicts the status quo. H7 is **decisively eliminated**. The current approach, while functional, has too many known deficiencies, unaddressed barriers, and strategic risks to sustain.

---

## Step 5: Hypothesis Survival Analysis

### Eliminated Hypotheses

| Hypothesis             | Disconfirming Evidence Count | Reason for Elimination                                                                                                                                                                                                                                                                   |
| ---------------------- | :--------------------------: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **H7: Status Quo**     |              19              | Overwhelming evidence from all 8 personas that the current approach has critical gaps, unaddressed adoption barriers, and strategic risks. Every persona independently identified improvements needed.                                                                                   |
| **H5: Full Migration** |              7               | Framework switches kill projects (historical evidence); migration effort disproportionate to market size; eliminates WINE's modification advantage; creates a novel, unproven split architecture. The Historian's warning against scope creep during migration is the decisive argument. |

### Weakened Hypotheses

| Hypothesis               | Disconfirming Evidence Count | Status                                                                                                                                                                                                                                   |
| ------------------------ | :--------------------------: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **H3: UI/UX First**      |        1 (but severe)        | Viable as complement, not primary strategy. WinForms ceiling limits UI investment returns. Best elements (setup wizard, documentation, accessibility) can be pursued independently.                                                      |
| **H6: Steam Deck Focus** |              3               | Viable as priority audience, not exclusive focus. Over-specialization invites absorption by general tools. Steam Deck emphasis should be within a broader Linux gaming strategy.                                                         |
| **H1: Technical Depth**  |              6               | Viable only in narrow form: injection method abstraction (swappable strategies) rather than deep perfection of any single method. Historical evidence shows distribution/community matters more than injection technique sophistication. |

### Surviving Hypotheses

| Hypothesis                      | Disconfirming Evidence Count | Strength Assessment                                                                                                                                                                                                                           |
| ------------------------------- | :--------------------------: | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **H4: Community Platform**      |              1               | Strongest survivor. Only contradicted by market size uncertainty (low-confidence estimate). Supported by 6 of 8 personas. Addresses the historically decisive success factor (distribution). Creates network effects that compound over time. |
| **H2: Multi-Tier Modification** |              0               | Strongest on technical dimension. Zero contradicting evidence. Universally consistent with historical evidence (simpler = more WINE-compatible), Contrarian's alternative approaches, and Archaeologist's tiered compatibility table.         |

---

## Relative Strength Assessment

### Ranked by Surviving Evidence Strength

**1. H4: Community Platform Strategy** -- Overall Strongest

- **For**: E9, E14, E15, E16, E17, E20, E26, E28 (8 consistent)
- **Against**: E3 (1 inconsistent, low confidence)
- **Ratio**: 8:1
- **Decisive evidence**: The Systems Thinker's identification of a compatibility database as the "single highest-leverage feature" is reinforced by the Analogist's documentation of how Lutris (6000+ community scripts), ProtonDB (millions of reports), and ROM hacking communities all grew through exactly this mechanism. The Historian's observation that "every successful trainer platform solved a distribution problem" provides the historical validation. The Negative Space Explorer's finding that "every user starts from zero" quantifies the waste that community sharing would eliminate.

**2. H2: Multi-Tier Modification System** -- Strongest Technical Strategy

- **For**: E1, E4, E5, E7, E8, E11, E20, E21, E22, E27 (10 consistent)
- **Against**: None
- **Ratio**: 10:0
- **Decisive evidence**: The Archaeologist's tiered injection compatibility table (config patching at 100% WINE compatibility, down to manual mapping at low compatibility) is the clearest statement. The Historian's "simpler is better under WINE" principle and the Contrarian's documentation of CreateRemoteThread unreliability under WINE together make a powerful case for diversifying modification approaches beyond DLL injection alone.

**3. H1: Injection Method Abstraction** (narrowed form) -- Supporting Technical Strategy

- **For**: E1, E8, E11, E13, E20, E21 (6 consistent)
- **Against**: E4, E5, E6, E7, E9, E17 (6 inconsistent)
- **Ratio**: 6:6
- **Status**: Deadlocked in full form. Viable only as injection method strategy pattern (swappable strategies, as the Analogist describes via Lutris's "runner" abstraction) rather than deep investment in any single method.

### Composite Strategy Recommendation

The evidence supports a **composite strategy** combining the strongest elements:

| Priority | Strategy Element                                                  | Source Hypothesis | Key Evidence  |
| :------: | ----------------------------------------------------------------- | :---------------: | :-----------: |
|    1     | Community profile manifests (JSON/TOML), sharing, import/export   |        H4         | E14, E15, E28 |
|    2     | Compatibility database (ProtonDB for trainers)                    |        H4         |    E16, E9    |
|    3     | Tiered modification: config patching + memory writes + injection  |        H2         | E7, E22, E21  |
|    4     | Injection method abstraction (strategy pattern, multiple methods) |   H1 (narrowed)   |    E1, E13    |
|    5     | Setup friction reduction (first-run wizard, auto-detection)       |        H3         |   E26, E27    |
|    6     | Steam Deck as priority (not exclusive) audience                   |        H6         |   E20, E19    |

---

## Step 6: Discriminating Evidence Needed

Several hypotheses remain difficult to distinguish without additional evidence:

### Between H4 (Community Platform) and H2 (Multi-Tier Modification)

| Discriminating Evidence                                        | Would Favor H4 If...                                                        | Would Favor H2 If...                                                                  | How to Obtain                                   |
| -------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- | ----------------------------------------------- |
| User survey: "What prevents you from using trainers on Linux?" | Setup complexity and discovery are top barriers                             | Injection failures and compatibility are top barriers                                 | Community survey on r/SteamDeck, r/linux_gaming |
| A/B test: profile sharing vs. config patching feature          | Profile sharing drives more new users                                       | Config patching enables more successful sessions                                      | Ship both, measure usage telemetry              |
| Competitor emergence                                           | A competing tool builds a community database first (validating market need) | A competing tool offers native injection alternatives (validating technical approach) | Monitor GitHub, Reddit, ProtonDB discourse      |

### Between H3 (UI/UX) and H5 (Migration)

| Discriminating Evidence                   | Would Favor H3 If...                                             | Would Favor H5 If...                                                               | How to Obtain                                                     |
| ----------------------------------------- | ---------------------------------------------------------------- | ---------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| WinForms NativeAOT maturity in .NET 11/12 | NativeAOT works well with WinForms, making migration unnecessary | NativeAOT requires non-WinForms UI, forcing migration anyway                       | Track .NET 11 previews (expected late 2026)                       |
| Avalonia proof-of-concept                 | Avalonia POC reveals unsolvable split-architecture problems      | Avalonia POC demonstrates clean separation of UI from engine                       | Build a time-boxed Avalonia spike (2-4 weeks)                     |
| WinForms accessibility audit              | WinForms MSAA/UIA is sufficient for meaningful accessibility     | WinForms accessibility hits hard limitations that only a native framework resolves | Conduct accessibility audit with screen reader testing under WINE |

### For validating H4's market size concern (E3)

| Discriminating Evidence                            | Source                                                    |
| -------------------------------------------------- | --------------------------------------------------------- |
| CrossHook GitHub star/download count trends        | GitHub Insights                                           |
| Steam hardware survey Linux percentage (2026 data) | store.steampowered.com/hwsurvey                           |
| SteamOS on third-party devices adoption rate       | Tech press, Steam forums                                  |
| Community response to a trial profile repository   | Publish 10 game profiles, measure downloads/contributions |

---

## Assumptions Challenged

The ACH process exposed several assumptions embedded in the research that warrant scrutiny:

### Assumption 1: "The WINE/Proton Approach Is Correct"

**Challenged by**: Contrarian (E4) -- native Linux mechanisms (LD_PRELOAD, ptrace) bypass WINE entirely.

**Assessment**: This assumption survives, but conditionally. The Historian's evidence that "native alternatives almost always lose to improved WINE compatibility" (the 80% phenomenon, Pattern 4) and the Archaeologist's finding that "native Linux memory scanners cannot scan WINE processes effectively" both support the WINE approach. However, the Contrarian's point about WINEDLLOVERRIDES and DLL proxy loading is valid -- these are WINE mechanisms, not native Linux mechanisms, and they should be part of H2's tiered approach.

**Verdict**: Partially challenged. CrossHook should remain a WINE application but incorporate WINE-native loading mechanisms (DLL overrides, proxy loading) alongside injection.

### Assumption 2: "The Market Exists and Is Worth Serving"

**Challenged by**: Contrarian (E3) -- the TAM may be only 15K-40K users.

**Assessment**: The Contrarian's estimate is explicitly labeled "Low confidence." Counter-evidence includes the Journalist's report of Steam Deck selling 5-10 million units, the Futurist's prediction of Linux market share reaching 4-6% by 2027, and the expansion of SteamOS to third-party handhelds. The Systems Thinker's Loop 1 (Proton Adoption Virtuous Cycle) provides the mechanism for market growth. The market is small today but growing.

**Verdict**: Partially challenged. The market is real but small. CrossHook's strategy must be proportionate to the market size -- avoid over-engineering, favor low-cost/high-leverage interventions (community profiles over architectural rewrites).

### Assumption 3: "WinForms Is Acceptable"

**Challenged by**: Contrarian (E2) -- WinForms under WINE has persistent rendering issues; Negative Space Explorer (E26) -- the UI contributes to the 13-step setup cliff.

**Assessment**: The Historian counters that "switching frameworks historically kills projects" (E12), and the Archaeologist notes that WinForms is "the pragmatic choice" that "game tool users expect game-like interfaces, not native-feeling ones." The Futurist identifies NativeAOT as a way to improve the WinForms experience (smaller binary, faster startup) without framework migration. The Negative Space Explorer notes that WinForms has better accessibility foundations than competitors.

**Verdict**: WinForms is acceptable for now. The framework should be improved (dark theme, accessibility, setup wizard) rather than replaced. Revisit this assumption when .NET 11-12 clarifies NativeAOT + WinForms maturity.

### Assumption 4: "DLL Injection Is the Core Value Proposition"

**Challenged by**: Archaeologist (E22) -- config patching is 100% WINE-compatible and requires no injection; Systems Thinker (E17) -- value shifts to convenience as Proton improves; Historian (E9) -- success comes from distribution, not technical capability.

**Assessment**: This is the most significantly challenged assumption. Multiple personas independently argue that CrossHook's value should not be centered on DLL injection alone. The Archaeologist's tiered approach (E21, E22), the Systems Thinker's convenience shift (E17), and the Negative Space Explorer's profile system analysis (E28) all point toward CrossHook's value being as a configuration/management platform that happens to support injection as one of several modification methods.

**Verdict**: Significantly challenged. DLL injection should be one capability among several, not the defining capability. The multi-tier modification system (H2) addresses this directly.

### Assumption 5: "Anti-Cheat Is Not CrossHook's Problem"

**Challenged by**: Systems Thinker (E5) -- anti-cheat increasingly operates in single-player modes; Contrarian -- even single-player games with anti-cheat are growing.

**Assessment**: The Systems Thinker explicitly states "anti-cheat is a boundary, not a problem to solve." The Historian notes that under WINE, kernel-mode anti-cheat does not function (E11). The Journalist confirms anti-cheat's Proton behavior is game-specific and often less restrictive than on Windows.

**Verdict**: Largely upheld. CrossHook should explicitly scope to games without active anti-cheat, document compatibility clearly, and let the compatibility database (H4) capture which games have anti-cheat issues. Not a problem to solve but a boundary to communicate.

---

## Key Insights

### Insight 1: The Distribution Problem Is the Real Problem

The Historian's observation (E9) that "every successful trainer tool generation solved a distribution problem, not just a technical one" is the single most important insight across all personas. GameCopyWorld solved discovery. WeMod solved installation. On Linux, neither problem is solved. CrossHook's community profile system (H4) is the mechanism to solve distribution for Linux trainers. Technical improvements (H1, H2) are necessary but not sufficient.

### Insight 2: Simplicity and WINE Compatibility Are Inversely Correlated with Technique Sophistication

The Archaeologist's injection compatibility table reveals a clear pattern: simpler modification techniques have higher WINE compatibility. Config patching (100%), direct memory writes (high), CreateRemoteThread (medium-high), manual mapping (low). This inverts the Windows-native assumption that more sophisticated equals better. CrossHook's optimal strategy is to offer the simplest technique that works for each game, not to push toward the most sophisticated technique possible.

### Insight 3: The Compatibility Database Is a Phase Transition Point

The Systems Thinker's analysis of network effects suggests that a compatibility database functions as a phase transition: below critical mass, it adds marginal value; above critical mass, it becomes self-sustaining and creates a defensible moat. ProtonDB demonstrated this transition with approximately 10,000 reports. CrossHook needs a lower threshold (given the smaller audience) but the same mechanism applies. The strategic question is not whether to build it, but how to reach critical mass with a small initial user base.

### Insight 4: CrossHook's WINE-Inside Architecture Is a Strength, Not a Limitation

The Historian (E11) and Archaeologist converge on a counterintuitive finding: running under WINE is an advantage for game modification because kernel-mode anti-cheat does not function in user-space WINE, and the modification tool sees the same address space as the game. Native Linux tools (scanmem, GameConqueror) cannot effectively scan WINE processes because they see the WINE process, not the Windows game inside it. CrossHook's architecture should be recognized and leveraged as a feature, not treated as a compromise.

### Insight 5: The Biggest Risk Is Not Technical but Organizational

The Contrarian's estimate of 15K-40K addressable users and the Systems Thinker's analysis of multiplicative maintenance burden together suggest that CrossHook's biggest risk is not a technical failure but organizational: building more than the market and maintainer capacity can sustain. The strategy must be proportionate. Community-contributed profiles (low maintainer burden, high value) are better investments than complex features (high maintainer burden, marginal value).

---

## Methodology Notes

### Process

1. All nine persona findings were read in full (Historian, Contrarian, Analogist, Systems Thinker, Journalist, Archaeologist, Futurist, Negative Space Explorer, plus the research objective).
2. Seven hypotheses were generated based on recurring themes across persona findings, ensuring mutual exclusivity on the primary dimension (where should CrossHook focus engineering and strategic effort?).
3. Twenty-eight pieces of evidence were selected from across all personas, weighted toward evidence with discriminating power (evidence that supports some hypotheses while contradicting others).
4. Each evidence-hypothesis pair was evaluated for consistency (C), inconsistency (I), or neutrality (N), with emphasis on identifying inconsistencies.
5. Hypotheses were assessed by counting and weighing disconfirming evidence, following ACH best practice that disconfirming evidence is more diagnostic than confirming evidence.

### Limitations

1. **No live web sources**: All persona findings are based on training data through May 2025, creating a 10-month gap (June 2025 through March 2026). Significant developments in Proton, WINE, WeMod, or the Steam Deck ecosystem during this period could alter the analysis.

2. **No user data**: CrossHook has no telemetry, user surveys, or download analytics. All claims about user behavior and adoption barriers are inferred from community patterns, not measured from CrossHook's actual user base.

3. **Confidence asymmetry**: The Contrarian's market size estimate (E3, "Low" confidence) is the primary disconfirming evidence against the strongest hypothesis (H4). This asymmetry means the analysis is sensitive to the accuracy of a low-confidence estimate.

4. **Persona overlap**: Several personas converge on the same recommendations (community database, profile improvements, tiered modification), which could reflect genuine multi-perspective validation or shared assumptions in the research methodology.

5. **Single-project focus**: The analysis examines CrossHook in isolation. Competitive dynamics with potential future tools are speculative.

---

## Confidence Assessment

| Analysis Element                                             |   Confidence    | Reasoning                                                                                                                                                                             |
| ------------------------------------------------------------ | :-------------: | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H7 (Status Quo) elimination                                  |    **High**     | 19 pieces of disconfirming evidence from all 8 personas. No reasonable interpretation supports status quo.                                                                            |
| H5 (Full Migration) elimination                              |    **High**     | 7 pieces of disconfirming evidence, including the Historian's high-confidence observation about framework migrations killing projects. Multiple independent lines of disconfirmation. |
| H4 (Community Platform) as strongest                         | **Medium-High** | Broadest support (6/8 personas), only one disconfirming evidence at low confidence. However, the critical mass problem is real and untested.                                          |
| H2 (Multi-Tier Modification) as strongest technical strategy |    **High**     | Zero disconfirming evidence. Multiple independent personas converge on tiered approaches through different analytical frameworks.                                                     |
| Composite strategy recommendation                            |   **Medium**    | Synthesizes multiple hypotheses, which introduces interpretation risk. The sequencing and prioritization is analytical judgment, not directly evidence-driven.                        |
| Market size sensitivity                                      | **Low-Medium**  | The Contrarian's 15K-40K estimate is explicitly low-confidence, but no higher-confidence estimate exists. The analysis is sensitive to this number.                                   |

---

## Appendix: Evidence Source Map

| Evidence | Historian | Contrarian | Analogist | Systems Thinker | Journalist | Archaeologist | Futurist | Negative Space |
| -------- | :-------: | :--------: | :-------: | :-------------: | :--------: | :-----------: | :------: | :------------: |
| E1       |           |     X      |           |                 |            |               |          |                |
| E2       |           |     X      |           |                 |            |               |          |                |
| E3       |           |     X      |           |                 |            |               |          |                |
| E4       |           |     X      |           |                 |            |               |          |                |
| E5       |           |     X      |           |        X        |            |               |          |                |
| E6       |           |     X      |           |        X        |            |               |          |                |
| E7       |     X     |            |           |                 |            |       X       |          |                |
| E8       |     X     |            |           |                 |            |               |          |                |
| E9       |     X     |            |           |                 |            |               |          |                |
| E10      |     X     |            |           |                 |            |               |          |                |
| E11      |     X     |            |           |                 |            |       X       |          |                |
| E12      |     X     |            |           |                 |            |               |          |                |
| E13      |           |            |     X     |                 |            |               |          |                |
| E14      |           |            |     X     |                 |            |               |          |                |
| E15      |           |            |     X     |                 |            |               |          |                |
| E16      |           |            |           |        X        |            |               |          |                |
| E17      |           |            |           |        X        |            |               |          |                |
| E18      |           |            |           |        X        |            |               |          |                |
| E19      |           |            |           |                 |     X      |               |          |                |
| E20      |           |            |           |                 |     X      |               |          |                |
| E21      |           |            |           |                 |            |       X       |          |                |
| E22      |           |            |           |                 |            |       X       |          |                |
| E23      |           |            |           |                 |            |               |    X     |                |
| E24      |           |            |           |                 |            |               |    X     |                |
| E25      |           |            |           |                 |            |               |          |       X        |
| E26      |           |            |           |                 |            |               |          |       X        |
| E27      |           |            |           |                 |            |               |          |       X        |
| E28      |           |            |           |                 |            |               |          |       X        |
