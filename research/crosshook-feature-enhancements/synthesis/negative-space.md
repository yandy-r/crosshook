# Negative Space Synthesis: What We Don't Know That Could Change Our Decisions

**Date**: 2026-03-19
**Input**: All 8 persona findings (Historian, Contrarian, Analogist, Systems Thinker, Journalist, Archaeologist, Futurist, Negative Space Explorer), Crucible Analysis (ACH), Contradiction Mapping
**Purpose**: Compile all gaps, uncertainties, and unanswered questions from the research. Prioritize by importance.

---

## Executive Summary

Across 8 persona findings and 2 synthesis documents, the research produced substantial architectural guidance and strategic direction for CrossHook. However, the research also exposed **significant blind spots** that could invalidate core assumptions if resolved differently than expected. The most consequential unknowns are: (1) the actual reliability rate of CreateRemoteThread injection under various Proton versions -- no one has ever measured this systematically; (2) the true size and growth trajectory of the target market, where estimates range from 15,000 to several hundred thousand depending on assumptions; (3) whether WINE's implementation of injection-related kernel32 APIs will improve, stagnate, or regress in future versions; and (4) whether a community profile-sharing system can reach critical mass in a niche market.

All 8 personas noted they lacked web search access, creating a 10-month knowledge gap (May 2025 to March 2026). During this window, .NET 10 shipped, new Proton versions likely released, WeMod may have changed its WINE/Linux stance, and the Steam Deck market evolved. Every finding relying on "current state" data should be considered stale until verified.

The research is strong on qualitative analysis but **entirely absent on quantitative data**. No persona produced actual numbers for injection success rates, user counts, performance benchmarks, or compatibility matrices. The strategic recommendations are built on estimates, analogies, and historical patterns -- not empirical measurement.

---

## Critical Unanswered Questions

### Question 1: What is the actual injection success rate under WINE/Proton?

**Why critical**: The Crucible Analysis identifies "Multi-Tier Modification System" as having zero disconfirming evidence, and the Contrarian calls CreateRemoteThread "the single largest technical risk." But neither claim is backed by measured data. If CreateRemoteThread actually succeeds 95%+ of the time for the games CrossHook targets, the urgency of alternative injection methods drops significantly. If it fails 30%+ of the time, the current architecture is fundamentally broken.

**Current status**: Zero systematic testing data exists. The Negative Space Explorer explicitly identifies this: "Nobody has published comprehensive test results for different injection techniques under different WINE/Proton versions." The Contrarian describes failure modes theoretically (address space differences, pressure-vessel isolation, timeout issues) but cites no failure rate data. The Journalist states CreateRemoteThread "works but behavior can differ from native Windows" without quantifying the difference.

**What's needed**:

- A test harness that performs CreateRemoteThread + LoadLibraryA injection against controlled target processes under multiple Proton versions (7.x, 8.x, 9.x, Experimental, GE-Proton)
- Success/failure tracking per Proton version, per target architecture (x86/x64), per injection timing (immediate, delayed, suspended-start)
- Comparison of alternative methods (DLL search order hijacking, WINEDLLOVERRIDES, Debug API attachment) against the same matrix
- Documentation of specific failure modes and error codes

**Priority**: P0 -- This is the single most important empirical gap. Every strategic decision about injection method diversification depends on knowing how often the current approach actually fails.

---

### Question 2: How many people actually use or attempt to use trainers on Linux/Steam Deck?

**Why critical**: The Contrarian estimates 15,000-40,000 total addressable users (self-rated "Low" confidence). The Futurist projects Linux gaming reaching 4-6% by 2027, potentially 8-12% by 2030. The Systems Thinker identifies market size as the key constraint on the "Community Platform" strategy -- the recommended primary strategy. If the market is truly 15K users, building community infrastructure is disproportionate. If it is 200K+, it becomes the highest-leverage investment.

**Current status**: No persona had access to CrossHook's actual download/usage numbers. No persona could verify Steam Deck sales figures (estimates ranged from 5-15 million across personas, self-contradictory within the Journalist's own findings). No persona could quantify the intersection of {Linux gamers} AND {trainer users}. The Contrarian's funnel calculation is the only attempt at quantification, and its author rates it "Low confidence." The Negative Space Explorer notes: "What nobody is measuring: Drop-off rates at each step. How many users download CrossHook but never successfully use it?"

**What's needed**:

- CrossHook GitHub download/clone/star statistics analysis
- Opt-in, privacy-respecting usage telemetry (even a simple launch counter)
- Community survey on r/SteamDeck, r/linux_gaming: "Have you tried using game trainers under Proton?"
- Steam Hardware Survey analysis for latest Linux market share
- Comparison with Lutris/Bottles/ProtonTricks usage data as proxy for "technically engaged Linux gamers"

**Priority**: P0 -- Resource allocation for every strategy (community platform, UI investment, architectural migration) depends on knowing how many users exist and how many could plausibly be reached.

---

### Question 3: How has the landscape changed in the 10-month knowledge gap (May 2025 - March 2026)?

**Why critical**: Every persona explicitly flagged that web search tools were unavailable. The Contradiction Mapping notes: "All eight personas plus the Negative Space Explorer were unable to execute live web searches. This means findings are based on knowledge through May 2025, with a 10-month gap." The Futurist predicted .NET 10 LTS shipping in November 2025 -- it likely has shipped, but no persona can confirm its actual features or WinForms NativeAOT status. Proton versions released since May 2025 may have changed injection API behavior.

**Current status**: A 10-month data gap. Specific unknowns include:

- Has .NET 10 shipped? What is its WinForms NativeAOT support status?
- What Proton versions have released since May 2025? Have they affected CreateRemoteThread behavior?
- Has WeMod taken any stance on WINE/Proton/Linux support?
- Has Valve announced Steam Deck 2?
- Have any competing Linux trainer tools emerged?
- Has the anti-cheat landscape changed (new games adding/removing anti-cheat)?
- Has SteamOS rolled out to third-party handhelds (Lenovo Legion Go S was announced)?

**What's needed**:

- Live web research to update all time-sensitive findings
- Specific verification of .NET 10 features, Proton changelog since 9.0, WeMod Linux stance, Steam Deck hardware/SteamOS updates
- Competitor scan for any new Linux trainer/mod tools

**Priority**: P0 -- The research conclusions may be significantly outdated. This is the cheapest gap to close (a few hours of web research) with potentially the highest impact on decision quality.

---

### Question 4: Is DLL proxy loading actually more reliable than CreateRemoteThread under WINE?

**Why critical**: The Historian and Archaeologist both advocate DLL proxy loading (placing a renamed DLL like dinput8.dll in the game directory) as "historically more reliable under WINE" and "the forgotten superior technique." The Contradiction Mapping flags this as a "priority disagreement" rather than a true contradiction. The Crucible Analysis recommends adding DLL proxy mode as Priority 3. But no persona provides measured reliability comparisons -- the claims are based on architectural reasoning (proxy loading uses WINE's normal DLL loader, which is well-tested) rather than empirical data.

**Current status**: Theoretical only. The Historian states the proxy approach "uses the normal DLL loading mechanism (no remote thread creation)" and "WINE's DLL loading behavior is well-tested because it is core functionality." The Archaeologist rates DLL search order hijacking as "High" WINE compatibility versus "Medium-High" for CreateRemoteThread. These are expert assessments, not measured data.

**What's needed**:

- Side-by-side testing of CreateRemoteThread injection versus DLL proxy deployment for the same set of target games under the same Proton versions
- Documentation of which games support which proxy DLL names (dinput8.dll, d3d9.dll, version.dll, etc.)
- Assessment of whether proxy DLL deployment conflicts with game integrity checks, Steam file verification, or anti-tamper systems

**Priority**: P1 -- This directly informs whether to invest in implementing the proxy DLL injection mode, which the Crucible Analysis recommends as Priority 3.

---

### Question 5: Can a community profile-sharing system reach critical mass with CrossHook's user base?

**Why critical**: The Crucible Analysis identifies "Community Platform Strategy" as the strongest surviving hypothesis, with 6 of 8 personas supporting it. The Systems Thinker calls a compatibility database the "single highest-leverage feature." But the Contrarian's market estimate (15K-40K users) raises the chicken-and-egg problem: community platforms need contributors to attract users and need users to attract contributors.

**Current status**: No persona provides evidence of a community platform succeeding at the scale CrossHook targets (niche-within-a-niche). The Analogist cites Lutris (6000+ community scripts) as precedent, but Lutris serves ALL Linux gamers, not just trainer users. ProtonDB serves all Proton users. The closest analogy might be LiveSplit auto-splitters (speedrunning community), but no persona quantifies that community's size relative to CrossHook's target. The Negative Space Explorer identifies "legal concerns about distributing trainer references" and "the effort of building community infrastructure" as barriers nobody has addressed.

**What's needed**:

- Analysis of comparable niche community platforms: How many active users did LiveSplit auto-splitters, ROM hacking patch databases, or Cheat Engine community tables need before becoming self-sustaining?
- A minimum viable community test: publish 10-20 well-documented profiles for popular games, publish them as a GitHub repository, and measure downloads and contributions over 3-6 months
- Legal review: Does linking to trainer download sites in profiles create liability?
- Assessment of contribution friction: How easy is it to create and submit a profile?

**Priority**: P1 -- This determines whether the recommended primary strategy (community platform) is viable or whether CrossHook should focus on a different competitive moat.

---

### Question 6: What is the actual performance impact of CrossHook on games?

**Why critical**: The Negative Space Explorer identifies this as an entirely avoided topic: "Nobody publishes benchmarks showing the performance impact of running a trainer alongside a game. Does DLL injection add frame time variance? Does memory monitoring cause stutters?" The Contrarian raises a related concern: ".NET 9 under WINE introduces its own problems: the CoreCLR runtime must initialize properly under WINE, JIT compilation paths differ." If CrossHook measurably degrades game performance, it undermines the entire value proposition for a tool targeting gaming devices with limited power (Steam Deck).

**Current status**: Zero performance data exists. No persona measured or estimated: startup time of CrossHook under WINE, memory footprint of CrossHook process, CPU impact of the 1000ms monitoring timer, frame time impact of DLL injection, or latency added by memory read/write operations.

**What's needed**:

- Benchmark CrossHook's startup time under Proton (with and without self-contained .NET 9 runtime)
- Measure CrossHook's memory footprint while monitoring a game
- Test frame time variance in a game with and without CrossHook's monitoring active
- Compare performance with NativeAOT compilation (when feasible) versus current JIT approach

**Priority**: P2 -- Important for user trust and Steam Deck viability, but unlikely to reveal a showstopper unless performance is egregiously bad.

---

### Question 7: What does WINE's Debug API implementation actually support?

**Why critical**: The Archaeologist identifies Debug API-based trainers as a "revival assessment" opportunity -- using DebugActiveProcess, hardware breakpoints, and WaitForDebugEvent as alternatives to CreateRemoteThread injection. The Contrarian suggests the Debug API has "different WINE compatibility characteristics than CreateRemoteThread." But the Archaeologist rates their WINE-specific assessment as only "Medium" confidence and states: "WINE's specific Debug API implementation quality needs verification."

**Current status**: Unknown. No persona verified WINE's Debug API coverage. The APIs in question (DebugActiveProcess, WaitForDebugEvent, SetThreadContext, GetThreadContext, and hardware debug registers DR0-DR7) are not commonly used by normal Windows applications, so they may receive lower WINE development priority. The Contrarian notes that "WINE developers have historically deprioritized trainer/injector compatibility" and "bugs in CreateRemoteThread or VirtualAllocEx that only affect DLL injectors/trainers are typically low-priority."

**What's needed**:

- Test WINE's implementation of DebugActiveProcess, WaitForDebugEvent, SetThreadContext, GetThreadContext
- Verify hardware breakpoint support (DR0-DR7) under WINE
- Compare Debug API approach reliability versus CreateRemoteThread for the same target processes
- Check WINE bugzilla/source code for known Debug API limitations

**Priority**: P2 -- Useful for expanding the injection method repertoire but not blocking for near-term roadmap decisions.

---

### Question 8: How do specific Proton versions affect existing CrossHook functionality?

**Why critical**: The Systems Thinker models a causal chain: "Valve releases new Proton version -> WINE base version changes -> Win32 API implementation details change -> CreateRemoteThread behavior may change." The Historian warns: "FLiNG trainers that worked under Proton 7.x have broken under Proton 8.x and 9.x due to changes in WINE's kernel32.dll implementation." But no persona has built the compatibility matrix that would let users (or CrossHook itself) know which Proton version to use for which game/trainer combination.

**Current status**: No compatibility matrix exists anywhere. The Negative Space Explorer states: "No tool or community resource systematically tracks which trainers work with which Proton versions." Individual data points exist in ProtonDB comments and Reddit threads, but no structured database.

**What's needed**:

- A test matrix: CrossHook injection against 10-20 popular trainable games, across Proton 7.x, 8.x, 9.x, and Experimental
- Documentation of which Proton versions introduce behavioral changes for injection-related APIs
- An ongoing regression testing process triggered by new Proton releases
- Ideally, a Docker/Podman-based CI test that verifies injection against controlled targets

**Priority**: P1 -- This directly feeds into the recommended community compatibility database and is the most concrete form of "quality data" CrossHook could produce.

---

### Question 9: What is the actual effort required to implement key proposed features?

**Why critical**: The Crucible Analysis recommends a 6-item composite strategy. The Analogist proposes 20+ transferable patterns. The Negative Space Explorer identifies at least 10 "missing features nobody is building." But no persona estimated implementation complexity or time for any proposed feature. The gap between "this pattern is structurally applicable" and "this can be implemented in CrossHook's codebase within a reasonable timeframe" is unquantified.

**Current status**: No implementation estimates exist. Specific unknowns include:

- How much work is migrating `.profile` flat files to JSON/TOML manifests?
- What is the LOE for a first-run setup wizard in WinForms under WINE?
- How complex is implementing DLL proxy deployment mode alongside existing injection?
- What infrastructure is needed for a community profile repository (GitHub-hosted vs. custom backend)?
- Can Steam library scanning (`libraryfolders.vdf` parsing) be done from within WINE, or does it require a native Linux companion?
- What is the LOE for adding accessibility properties to MainForm.cs?

**What's needed**:

- Spike/prototype for each Priority 1-3 recommendation from the Crucible Analysis
- Time-boxed proof-of-concept (2-4 hours each) for: JSON profile format, Steam library detection, DLL proxy deployment, first-run wizard
- Architecture review of whether proposed features require changes to the current project structure or can be additive

**Priority**: P1 -- Without implementation estimates, the roadmap is aspirational rather than actionable.

---

### Question 10: What is Valve's actual stance on trainer/modding tools under Proton?

**Why critical**: The Systems Thinker notes Valve could "restrict Proton capabilities that CrossHook depends on, though this is philosophically unlikely given Valve's open platform stance." The Contrarian warns about "Pressure-vessel isolation" and notes "Tools that attempt cross-process manipulation are working against the security model that Valve is building." These are opposing predictions about a single actor with outsized power over CrossHook's viability.

**Current status**: Ambiguous. Valve has historically been pro-modding (Steam Workshop). Valve partnered with EAC and BattlEye for Proton support, which was about enabling games, not restricting modification. The Systems Thinker rates Valve's stance as "philosophically unlikely" to restrict modification. But no persona cited any official Valve statement about trainer/modifier tools under Proton. Valve's pressure-vessel containerization is a security measure that could incidentally break CrossHook's cross-process injection.

**What's needed**:

- Review of Valve developer documentation, Steamworks announcements, or SteamOS release notes for any statements about third-party process modification tools
- Analysis of pressure-vessel changes across recent Proton versions and their impact on cross-process operations
- Monitoring of SteamOS security model changes that could affect tool permissions

**Priority**: P2 -- Important for strategic risk assessment but unlikely to yield a definitive answer (Valve rarely makes public statements about edge-case use cases).

---

## Research Gaps by Category

### Empirical Gaps (No Data Exists)

| Gap                                                                 | Identified By                 | Impact on Decisions                                                            |
| ------------------------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------ |
| Injection success rates per Proton version                          | Contrarian, Negative Space    | Determines urgency of alternative injection methods                            |
| Actual CrossHook user count and funnel drop-off                     | Negative Space, Contrarian    | Determines appropriate investment level for all strategies                     |
| Performance benchmarks (CrossHook + game under WINE)                | Negative Space                | Determines whether performance optimization is needed                          |
| WINE Debug API implementation coverage                              | Archaeologist                 | Determines viability of Debug API as fallback injection method                 |
| Steam Deck user demographics (modding interest, technical skill)    | Negative Space                | Determines UI/UX strategy and documentation level needed                       |
| DLL proxy loading reliability vs. CreateRemoteThread under WINE     | Historian, Archaeologist      | Determines priority of proxy DLL mode implementation                           |
| Minimum community size for self-sustaining profile contributions    | Systems Thinker (implied)     | Determines viability of community platform strategy                            |
| WinForms accessibility audit results under WINE                     | Negative Space                | Determines whether accessibility is a realistic differentiator                 |
| Binary size and startup time: current vs. NativeAOT                 | Futurist                      | Determines priority of NativeAOT migration                                     |
| Community survey: "What prevents you from using trainers on Linux?" | Crucible Analysis (suggested) | Discriminates between H4 (Community Platform) and H2 (Multi-Tier Modification) |

### Theoretical Gaps (No Frameworks Exist)

| Gap                                                                          | Why It Matters                                                    |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| No model of injection reliability as a function of WINE version              | Cannot predict when Proton updates will break CrossHook           |
| No framework for predicting game compatibility with injection tiers          | Cannot auto-recommend the right injection method per game         |
| No taxonomy of trainer tool failure modes under WINE                         | Cannot build systematic error handling or user-facing diagnostics |
| No model of community platform critical mass for niche tools                 | Cannot predict whether profile-sharing investment will pay off    |
| No framework for assessing WINE API coverage for non-standard usage patterns | Cannot systematically evaluate alternative injection techniques   |
| No model of how Proton prefix isolation affects cross-process operations     | Cannot design prefix management features without trial and error  |

### Practical Gaps (No Implementation Knowledge Exists)

| Gap                                                     | What Is Unknown                                                                                                                                                   |
| ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Tiered injection fallback implementation                | How to detect that Tier N failed and automatically try Tier N-1, including timing and error detection                                                             |
| Community profile repository system                     | Infrastructure decisions (GitHub-hosted vs. custom), moderation model, contribution workflow, legal implications                                                  |
| Automatic Proton prefix detection and configuration     | Whether Steam library files are readable from within WINE, how to detect Wine-Mono vs .NET Framework in a prefix, how to programmatically configure DLL overrides |
| Split architecture (native Linux UI + WINE engine)      | How to manage two processes, IPC protocol, startup sequence, error propagation, packaging                                                                         |
| WinForms NativeAOT readiness for CrossHook specifically | Which CrossHook patterns (reflection, COM, dynamic P/Invoke) block NativeAOT compilation                                                                          |
| DLL proxy deployment mode                               | Which proxy DLL names work for which games, how to generate forwarding exports, how to manage multiple proxy DLLs per game                                        |
| Steam library scanning from WINE context                | Whether `libraryfolders.vdf` and `appmanifest_*.acf` files are accessible and parseable from within a WINE process                                                |

---

## Evidence Limitations

### The Pervasive Web Search Gap

Every persona flagged the absence of web search tools. The Contrarian listed 10 specific queries it could not execute. The Journalist structured its entire methodology around a caveat that findings are "based on knowledge through May 2025." This is not a minor limitation -- it is the single largest weakness of the entire research effort.

**Specific findings most affected**:

| Finding                                             | Why Web Search Would Help                                                     |
| --------------------------------------------------- | ----------------------------------------------------------------------------- |
| "WeMod has no Linux support"                        | WeMod may have changed its stance in the last 10 months                       |
| "Steam Deck has sold 5-15 million units"            | Updated sales data would resolve internal contradictions                      |
| ".NET 10 is expected in November 2025"              | .NET 10 has likely shipped; its actual features matter for NativeAOT strategy |
| "Proton 9.0 is the latest major release"            | Newer Proton versions may have changed injection API behavior                 |
| "No competing Linux trainer tool exists"            | A competitor may have emerged                                                 |
| "Anti-cheat increasingly operates in single-player" | The trend may have accelerated or reversed                                    |
| "WINE's CreateRemoteThread implementation is buggy" | Recent WINE commits may have addressed known issues                           |
| "SteamOS expanding to third-party handhelds"        | Actual rollout status would affect market size estimates                      |

### Reliance on Expert Knowledge vs. Empirical Evidence

The research is heavily weighted toward expert reasoning and historical analogy rather than empirical measurement. The strongest claims are architectural assessments (e.g., "DLL proxy loading uses WINE's well-tested DLL loader path, therefore it should be more reliable") rather than measured outcomes (e.g., "DLL proxy loading succeeded in 47 of 50 test cases across 5 Proton versions"). This is a systematic bias inherent to desk research without testing infrastructure.

### Confidence Rating Inflation

Multiple personas self-rate "High" confidence on claims that are architecturally reasonable but empirically unverified. For example, the Archaeologist rates DLL proxy loading as "High" WINE compatibility based on architectural reasoning, not testing. The Contrarian rates CreateRemoteThread unreliability as "High" based on WINE architecture knowledge, not failure rate data. These ratings reflect the strength of the reasoning, not the strength of the evidence.

### Single-Source Dependency for Market Estimates

All market size estimates derive from the same data sources: Steam Hardware Survey (~2% Linux share) and unverified Steam Deck sales estimates. No persona had access to CrossHook-specific usage data, Linux gaming community survey results, or trainer download statistics from FLiNG/WeMod. The Contrarian's 15K-40K estimate and the Futurist's growth projections are both constructed from the same uncertain inputs.

### Absence of User Voice

No persona conducted user interviews, surveys, or usability testing. The Negative Space Explorer identifies "users who gave up" as the largest invisible stakeholder group. The Journalist characterizes community requests based on remembered Reddit patterns, not systematic content analysis. All user-need assessments are inferred from community observation rather than direct measurement.

---

## Key Insights

### Insight 1: The Research Is Strongest on "What" and "Why" but Weakest on "How Much"

The 8 personas collectively produced excellent qualitative analysis: what patterns exist, why they matter, what alternatives are available, and what risks are present. But the research produced zero quantitative measurements: how often does injection fail, how many users exist, how much does performance degrade, how long would features take to implement. Every strategic recommendation is built on unmeasured quantities. The first priority should be acquiring numbers, not building features.

### Insight 2: The 10-Month Knowledge Gap May Invalidate Key Assumptions

The gap between May 2025 (training data cutoff) and March 2026 (research date) is large enough that several key assumptions may have changed. .NET 10 LTS has likely shipped. New Proton versions have released. WeMod may have made moves toward or against Linux support. Steam Deck 2 may have been announced. The research was conducted on stale data, and the first step should be updating the most time-sensitive findings via live web research.

### Insight 3: The Crucible Analysis's Composite Strategy Rests on Unverified Premises

The recommended composite strategy (Community Platform + Multi-Tier Modification + Injection Abstraction + Setup Friction Reduction) is logically coherent and well-argued. But its key premises are unverified:

- "Community profiles will generate network effects" -- unverified (minimum viable community size unknown)
- "Tiered modification will improve reliability" -- unverified (no comparative testing data)
- "CreateRemoteThread is unreliable enough to justify alternatives" -- unverified (no failure rate data)
- "The market is growing enough to justify platform investment" -- unverified (no current market data)

Each of these premises could be cheaply tested before committing significant engineering effort.

### Insight 4: The Most Impactful Near-Term Action Is Not Building -- It Is Measuring

Given the empirical gaps, the highest-leverage near-term activity is not implementing features but:

1. Running live web research to close the 10-month knowledge gap (hours, not days)
2. Building a simple injection test harness to measure CreateRemoteThread success rates across Proton versions (days, not weeks)
3. Analyzing CrossHook's GitHub metrics for actual user interest signals (hours)
4. Publishing a community survey to understand user needs and barriers (days)

These measurements would convert the research from "well-reasoned but unverified" to "evidence-based and actionable."

### Insight 5: The Contrarian Was Right About the Problems, the Crucible Was Right About the Solutions, but Neither Had the Data to Prove It

The Contradiction Mapping observes: "The Contrarian is more right about the problems than about the solutions." The Crucible Analysis produces the most actionable synthesis. But the Contrarian's problem identification (multiplicative failure, small market, architectural ceiling) and the Crucible's solution recommendation (community platform, tiered modification) are both arguing from first principles rather than evidence. The missing data -- injection reliability, market size, community viability -- is the arbiter that would resolve their disagreement definitively.

---

## Priority-Ordered Action Items for Closing Gaps

| Priority | Action                                                                                                |     Effort     |   Impact   | Closes Gap                                 |
| :------: | ----------------------------------------------------------------------------------------------------- | :------------: | :--------: | ------------------------------------------ |
|    P0    | Live web research: .NET 10 status, Proton changes, WeMod stance, Steam Deck 2, competitors            |     Hours      |    High    | Q3 (knowledge gap)                         |
|    P0    | Analyze CrossHook GitHub stars, downloads, traffic, issues for user signals                           |     Hours      |   Medium   | Q2 (market size)                           |
|    P0    | Build minimal injection test harness; measure CreateRemoteThread success rate across Proton 7/8/9/Exp |      Days      |  Critical  | Q1 (injection reliability)                 |
|    P1    | Publish 10 game profiles to a test GitHub repository; measure adoption over 1-3 months                | Days + waiting |    High    | Q5 (community viability)                   |
|    P1    | Spike: JSON profile format migration from flat-file .profile                                          |     Hours      |   Medium   | Q9 (implementation effort)                 |
|    P1    | Spike: Steam library detection from within WINE process                                               |     Hours      |   Medium   | Q9 (implementation effort)                 |
|    P1    | Spike: DLL proxy deployment for 3 test games under Proton                                             |      Days      |    High    | Q4 (proxy reliability)                     |
|    P2    | Performance benchmark: CrossHook + game under Proton                                                  |     Hours      |   Medium   | Q6 (performance impact)                    |
|    P2    | Test WINE Debug API coverage for DebugActiveProcess, hardware breakpoints                             |     Hours      | Low-Medium | Q7 (Debug API viability)                   |
|    P2    | Build Proton version x game x trainer compatibility matrix for 10 games                               |      Days      |    High    | Q8 (Proton compatibility)                  |
|    P2    | Community survey on r/SteamDeck, r/linux_gaming                                                       |      Days      |   Medium   | Q2 (market size), Q5 (community viability) |
|    P3    | Avalonia UI proof-of-concept with split architecture (2-4 week spike)                                 |     Weeks      |   Medium   | Crucible recommendation                    |
|    P3    | WinForms accessibility audit with screen reader under WINE                                            |     Hours      |    Low     | Q9, Negative Space                         |

---

## What Would Change If Key Questions Were Answered

| If We Learn...                                                 | Impact on Strategy                                                                    |
| -------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Injection succeeds >90% across Proton versions                 | De-prioritize alternative injection methods; focus on community platform and UX       |
| Injection fails >25% across Proton versions                    | Elevate multi-tier modification to P0; DLL proxy mode becomes critical path           |
| Market is >100K addressable users                              | Justify community infrastructure investment; consider dedicated backend               |
| Market is <20K addressable users                               | Keep infrastructure minimal; Git-based profiles only; focus on core reliability       |
| .NET 10 has full WinForms NativeAOT support                    | Elevate NativeAOT migration to P0; immediate binary size and startup improvement      |
| .NET 10 WinForms NativeAOT is still experimental               | Defer NativeAOT; focus on features within current runtime                             |
| WeMod has announced Linux support                              | CrossHook's niche shrinks; differentiate on open-source, offline, power-user features |
| WeMod has not changed stance on Linux                          | CrossHook's positioning remains strong; community platform strategy validated         |
| A competing Linux trainer tool has emerged                     | Analyze competitor's approach; accelerate differentiating features                    |
| Proton 10.x breaks CreateRemoteThread behavior                 | Tiered modification becomes existentially urgent                                      |
| DLL proxy loading tests show >95% success rate                 | DLL proxy becomes recommended default; CreateRemoteThread becomes fallback            |
| Community profile repository gets <5 contributions in 3 months | Re-evaluate community platform strategy; focus on solo-user features                  |

---

## Conclusion

The research produced a coherent strategic direction -- Community Platform + Multi-Tier Modification -- that is well-argued across 8 persona perspectives and validated through rigorous hypothesis testing in the Crucible Analysis. However, this direction rests on premises that no persona could verify empirically. The negative space is not exotic or speculative; it consists primarily of straightforward measurements that have never been taken. Before committing to the recommended roadmap, the most responsible next step is a short empirical validation phase: close the knowledge gap with live web research, measure injection reliability, assess market signals, and prototype the highest-priority features to estimate implementation costs. The research has told us what to think about. The measurements will tell us what to do.
