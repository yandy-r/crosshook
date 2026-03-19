# Evidence Verification Log

**Date**: 2026-03-19
**Scope**: Cross-referencing claims made by multiple personas

---

## High-Confidence Findings

### Finding 1: Community profile sharing is the highest-leverage feature

- **Claimed by**: Systems Thinker, Analogist, Negative Space Explorer, Journalist, Historian, Archaeologist
- **Verification status**: Confirmed by convergence (6 of 8 personas, independent reasoning paths)
- **Confidence**: High
- **Notes**: Each persona arrived via different methodology (network effects, cross-domain analogy, absence detection, market gap, historical precedent, scaling analysis)

### Finding 2: CreateRemoteThread + LoadLibraryA is the pragmatic injection sweet spot under WINE

- **Claimed by**: Historian, Journalist, Archaeologist
- **Contradicted by**: Contrarian (calls it "single largest technical risk")
- **Resolution**: Both are correct in context. CRT is the best available _runtime injection_ method (Archaeologist's comparison table shows worse WINE compatibility for more advanced techniques), but it carries real risks (Contrarian's failure mode analysis). Resolution: multi-tier approach where CRT is Tier 2, not the only option.
- **Verification status**: Partially confirmed
- **Confidence**: Medium-High

### Finding 3: No trainer tool prioritizes Linux/Proton as first-class platform

- **Claimed by**: Journalist, Negative Space Explorer, Historian
- **Verification status**: Confirmed (consistent across personas; WeMod, FLiNG, CheatEngine, Plitch all Windows-primary)
- **Confidence**: High (but 10-month knowledge gap may mean a competitor has emerged)

### Finding 4: WinForms has unexploited accessibility advantage (UI Automation/MSAA)

- **Claimed by**: Negative Space Explorer
- **Verification status**: Confirmed by architecture (WinForms inherits .NET accessibility framework; no competing trainer tool has implemented accessibility)
- **Confidence**: High (architectural fact)

### Finding 5: Simpler injection techniques have higher WINE compatibility

- **Claimed by**: Archaeologist (tiered table), Historian, Contrarian (implicitly)
- **Verification status**: Confirmed by architectural reasoning (fewer translated API calls = fewer failure points)
- **Confidence**: High (principle validated cross-domain by Pattern Recognizer)

### Finding 6: Framework migrations historically kill projects

- **Claimed by**: Historian (5 examples: Amarok, GNOME 3, Angular, Python 2→3, EasyHook)
- **Verification status**: Confirmed (well-documented historical pattern)
- **Confidence**: High

---

## Medium-Confidence Findings

### Finding 7: Market size is 15,000-40,000 users

- **Claimed by**: Contrarian (self-rated "Low confidence")
- **Contradicted by**: Futurist (projects growth), Journalist (SteamOS expansion)
- **Verification status**: Unverified -- no empirical data exists
- **Confidence**: Low (acknowledged by its own author)
- **Notes**: This is the single weakest link in the research; all strategic decisions are sensitive to this number

### Finding 8: DLL proxy loading is more reliable than CreateRemoteThread under WINE

- **Claimed by**: Historian, Archaeologist
- **Verification status**: Not empirically verified -- based on architectural reasoning
- **Confidence**: Medium
- **Notes**: Needs side-by-side testing to confirm

### Finding 9: Split architecture (native UI + WINE engine) is inevitable

- **Claimed by**: Futurist, Negative Space Explorer, Analogist, Contrarian (each proposing fragments)
- **Pattern evidence**: Every surviving WINE-based tool has made this split (yabridge, Bottles, CrossOver, Heroic)
- **Verification status**: Confirmed by historical pattern
- **Confidence**: Medium-High

---

## Contradictions Requiring Resolution

### Contradiction 1: CreateRemoteThread reliability

- **Position A (Journalist/Historian)**: "LoadLibraryA/CreateRemoteThread is the right foundation for WINE"
- **Position B (Contrarian)**: "The single largest technical risk"
- **Evidence for A**: 25-year technique durability, Archaeologist's comparison showing worse WINE compat for alternatives
- **Evidence for B**: Architectural analysis of WINE's imperfect API implementation, failure modes documented
- **Resolution**: Multi-tier system where CRT is one option among several, not the sole approach. Both positions valid in different contexts.

### Contradiction 2: Market viability

- **Position A (Journalist/Systems Thinker)**: Viable niche with growth trajectory (Steam Deck, SteamOS expansion)
- **Position B (Contrarian)**: Market too small (15K-40K) for significant investment
- **Resolution**: Unknown -- requires empirical measurement. The Contrarian's estimate is self-rated "Low confidence." Market size is partially a function of CrossHook's own UX quality (reducing friction expands addressable market).

### Contradiction 3: WinForms assessment

- **Position A (Contrarian/Futurist)**: UX liability, deprecated, migration needed
- **Position B (Negative Space)**: Accessibility advantage via UI Automation
- **Position C (Journalist)**: Adequate for now
- **Resolution**: Context-dependent -- WinForms is simultaneously deprecated (Microsoft), a WINE liability (rendering), and an accessibility advantage (UI Automation). All three are true.
