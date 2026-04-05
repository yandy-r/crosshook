# ML-Assisted Configuration: Recommendations & Risk Assessment

## Executive Summary

The "ML-assisted configuration" feature (issue #77, P3/Very High effort/Low impact) is significantly de-risked by the already-shipped ProtonDB lookup infrastructure (issue #53, CLOSED). The existing `protondb` module in `crosshook-core` already fetches report feeds, extracts environment variables from freeform text via regex/heuristics, aggregates recommendations by frequency, and caches results with TTL. The "Very High" effort rating assumed none of this existed. The actual remaining effort for a heuristic-only approach is **Medium**, and the ML component should be deferred indefinitely -- the heuristic pipeline already delivers 80%+ of the value. The smartest strategy is incremental enhancement of existing infrastructure, not a new ML system.

---

## Implementation Recommendations

### Phasing Strategy: Incremental Enhancement Over New System

The critical insight is that the existing ProtonDB infrastructure already performs the core work this feature describes. The phasing strategy should build on what exists rather than creating parallel systems.

**Phase 0 -- Catalog Bridge (Low effort, High value)**

Map existing `ProtonDbEnvVarSuggestion` outputs to known `OptimizationCatalog` entry IDs. The optimization catalog has 25 entries with well-defined env var pairs. A simple lookup table (env key+value -> catalog ID) converts "community-reported env vars" into "actionable optimization toggles" the user already understands from the Launch tab.

- Input: `ProtonDbRecommendationGroup.env_vars` (already populated by `aggregation.rs`)
- Output: List of `OptimizationEntry.id` values that match community reports
- New code: ~100 lines in `crosshook-core`, a mapping function
- No new tables, no new caching, no new API calls

**Phase 1 -- Apply-to-Profile UI (Medium effort, High value)**

Add "Apply suggestion" actions to the existing ProtonDB lookup card (`ProtonDbLookupCard.tsx`). When the user views ProtonDB recommendations for a game, env var suggestions that map to known catalog entries show an "Enable" toggle. Suggestions that don't map to known entries show a "Copy to custom env vars" action.

- Frontend: Extend `ProtonDbLookupCard` with apply actions
- Backend: New Tauri command `apply_protondb_suggestion` that calls `ProfileStore::save_launch_optimizations()`
- Reuses existing `useProtonDbLookup` hook and `ProtonDbRecommendationGroup` types

**Phase 2 -- Enhanced Aggregation (Medium effort, Medium value)**

Improve the existing `normalize_report_feed()` aggregation with:

- Proton version weighting (prefer suggestions from recent Proton versions)
- Tier-weighted scoring (suggestions from Platinum-rated reports rank higher)
- Conflict detection (warn if two suggestions contradict each other)
- These are refinements to `aggregation.rs`, not a new system

**Phase 3 -- Suggestion Tracking (Low effort, Low value)**

If Phase 1-2 demonstrate user adoption, add lightweight tracking:

- Record which suggestions were applied per profile (a new field in `config_revisions` or a simple `suggestion_actions` table)
- Show "Previously applied" state in the ProtonDB card
- Only build this if usage data justifies it

**Phase 4 -- ML Model (Very High effort, speculative value)**

Defer indefinitely. Only pursue if:

- Phases 0-2 are live and generating usage data
- The heuristic approach demonstrably fails for a measurable class of games
- A pre-trained model becomes available (e.g., community-trained on ProtonDB corpus)
- The project has sufficient maintainer bandwidth for model versioning and inference infrastructure

### Minimum Viable Version

The minimum viable version is **Phase 0 + Phase 1**: a catalog bridge function and "Apply" buttons on the existing ProtonDB lookup card. This requires:

- ~100 lines of new Rust (mapping function in `crosshook-core`)
- ~1 new Tauri command
- ~200 lines of new TypeScript (UI actions in existing component)
- No new database tables
- No new API calls
- No ML infrastructure

**Estimated effort**: Low-Medium (compared to the original "Very High" rating)

### Technology Choices

- **No ML framework needed** for Phases 0-2. The existing regex + frequency heuristic in `aggregation.rs` is sufficient.
- **No new HTTP clients**. The existing `protondb_http_client()` handles all ProtonDB communication.
- **No new caching layer**. `external_cache_entries` with the existing `protondb:{app_id}` cache key already stores recommendation data.
- **If ML is ever pursued**: consider `ort` (ONNX Runtime for Rust) for local inference of a pre-trained model, avoiding Python runtime dependencies. Model files would live on the filesystem, not in SQLite.

---

## Improvement Ideas

### Related Features That Benefit From the Same Infrastructure

1. **Optimization catalog enrichment**: The catalog bridge (Phase 0) creates a mapping between ProtonDB community data and the optimization catalog. This mapping could also power "popularity" badges on optimization entries -- "Used by 47 ProtonDB reporters for this game."

2. **Community profile auto-suggestions**: When creating a new profile via auto-populate, the catalog bridge could pre-select optimizations that ProtonDB data suggests. This connects issue #77 to the existing auto-populate flow.

3. **Trainer version correlation (issue #58, P1)**: ProtonDB reports include `proton_version` fields. The same aggregation infrastructure could track which Proton versions produce the most Platinum/Gold reports per game, informing the version correlation feature.

4. **Configuration history (issue #59)**: If suggestion tracking (Phase 3) is built, it naturally feeds into the config history/rollback feature by recording why an optimization was enabled ("Applied from ProtonDB suggestion, 23 reports").

5. **PCGamingWiki integration**: A future data source that could feed into the same suggestion pipeline. PCGamingWiki has structured data (not freeform text), making extraction more reliable but requiring a different client. The catalog bridge and apply-to-profile UI would be reusable.

### Connections to Existing Infrastructure

| Existing Component        | Connection to This Feature                                                                 |
| ------------------------- | ------------------------------------------------------------------------------------------ |
| `protondb/aggregation.rs` | Already extracts env vars and ranks by frequency -- the core of "ML-assisted" suggestions  |
| `protondb/models.rs`      | `ProtonDbEnvVarSuggestion` and `ProtonDbRecommendationGroup` already model suggestion data |
| `launch/catalog.rs`       | `OptimizationCatalog` defines the target vocabulary for mapping suggestions                |
| `profile/toml_store.rs`   | `save_launch_optimizations()` is the write path for applying suggestions                   |
| `ProtonDbLookupCard.tsx`  | Existing UI that displays recommendations -- extend with apply actions                     |
| `useProtonDbLookup.ts`    | Existing hook that fetches and normalizes ProtonDB data                                    |
| `external_cache_entries`  | Already caches ProtonDB responses with 6-hour TTL                                          |

---

## Risk Assessment

### Technical Risks

| Risk                                           | Severity | Likelihood | Mitigation                                                                                                                                         |
| ---------------------------------------------- | -------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| ProtonDB API changes break report feed hash    | High     | Medium     | Existing code already handles this with retry + stale cache fallback; add monitoring for increased 404 rates                                       |
| Env var extraction misses valid suggestions    | Medium   | Low        | The regex approach catches standard `KEY=VALUE %command%` patterns; edge cases (quoted values, multi-line) are intentionally excluded for safety   |
| Extracted suggestions break specific games     | High     | Low        | Never auto-apply; always show as optional with "X reports" attribution; leverage existing `is_safe_env_value()` validation                         |
| ProtonDB rate-limits or blocks requests        | Medium   | Medium     | Existing 6-hour cache TTL limits request frequency; add exponential backoff for 429 responses                                                      |
| Mapping gaps between ProtonDB vars and catalog | Low      | High       | Expected: many ProtonDB suggestions won't map to known catalog entries; show these as "copy-only" raw suggestions (already handled by existing UI) |
| Stale suggestions after game updates           | Medium   | Medium     | ProtonDB reports include timestamps; weight recent reports higher in Phase 2                                                                       |

### Integration Challenges

1. **Profile mutation from the ProtonDB card**: The current ProtonDB lookup is read-only. Adding "Apply" actions requires the ProtonDB card to mutate profile state, which means it needs access to the profile save path. This is a new coupling between the ProtonDB display layer and the profile store.

2. **Optimization ID stability**: The catalog bridge maps env vars to catalog IDs. If catalog IDs change (e.g., `disable_esync` is renamed), saved suggestions break. Mitigation: catalog IDs are stable by convention and validated at load time.

3. **Frontend state synchronization**: Applying a suggestion updates `LaunchOptimizationsSection.enabled_option_ids`. The optimization toggles UI on the Launch tab must reflect this change without a full page reload. Requires either lifting state or using Tauri events.

### The "ML vs Regex" Question

**Verdict: Regex/heuristic is sufficient. ML is not justified for this feature at P3 priority.**

Evidence:

- The existing `safe_env_var_suggestions()` already parses `KEY=VALUE` pairs from the pre-`%command%` portion of launch options. This covers the dominant pattern in ProtonDB reports.
- Report count weighting (already implemented) provides a confidence signal equivalent to what a simple ML classifier would produce.
- The primary failure mode of heuristics (missing complex multi-line launch options) is also a safety boundary -- complex launch strings are more likely to contain dangerous commands.
- An ML model would require: training data curation, model hosting, inference pipeline, versioning, and ongoing retraining. This is a disproportionate investment for a P3/Low-impact feature.
- If heuristics prove insufficient for specific games, a curated override file (like the optimization catalog TOML) is simpler and more maintainable than an ML model.

---

## Alternative Approaches

### Option A: Enhance Existing ProtonDB Card (Recommended)

Add apply actions to the existing `ProtonDbLookupCard.tsx`. Map env var suggestions to optimization catalog entries. No new modules, tables, or APIs.

- **Pros**: Minimal new code, builds on proven infrastructure, ships fast, low maintenance
- **Cons**: Limited to ProtonDB data quality, no learning/feedback loop
- **Effort**: Low-Medium
- **Recommendation**: **Do this first.** It delivers the core value proposition with minimal investment.

### Option B: Pure Display (No Extraction)

Show raw ProtonDB data without extraction or actionable suggestions. The existing `ProtonDbLookupCard` already does this -- env vars and launch options are displayed with report counts.

- **Pros**: Zero new code (already shipped)
- **Cons**: Users must manually interpret and apply suggestions; no progress toward the issue's acceptance criteria
- **Effort**: None (already done)
- **Recommendation**: This is the current state. Phase 0-1 builds incrementally on it.

### Option C: Community-Curated Configs

Instead of extracting from ProtonDB, maintain curated per-game configuration presets in community taps (TOML files).

- **Pros**: High quality, human-verified, no API dependency
- **Cons**: Requires active community curation, doesn't scale to long tail of games, maintenance burden shifts to community
- **Effort**: Medium (tap format extension + UI)
- **Recommendation**: Complementary to Option A, not a replacement. Could be Phase 2.5.

### Option D: PCGamingWiki Integration

Use PCGamingWiki's structured data (Cargo tables with key-value fields) instead of ProtonDB freeform text.

- **Pros**: Structured data is more reliable than regex extraction; broader coverage (not just Proton)
- **Cons**: Requires HTML scraping or Cargo API client; different data model; PCGamingWiki focuses on fixes/workarounds rather than optimizations
- **Effort**: High (new client, new parsing, new data model)
- **Recommendation**: Future data source that could feed into the same catalog bridge. Not a replacement for ProtonDB integration.

### Option E: Full ML Pipeline

Train an ML model on the ProtonDB corpus to predict optimal configurations per game.

- **Pros**: Could discover non-obvious correlations; improves over time
- **Cons**: Very high effort, requires training infrastructure, model versioning, inference pipeline; disproportionate to P3 priority and Low impact rating; adds maintenance burden for a solo maintainer
- **Effort**: Very High
- **Recommendation**: **Defer indefinitely.** Only revisit if Phases 0-2 prove insufficient and the project gains additional maintainers.

---

## Task Breakdown Preview

### Phase 0: Catalog Bridge (1-2 tasks)

| Task | Description                                                                                     | Dependencies |
| ---- | ----------------------------------------------------------------------------------------------- | ------------ |
| 0.1  | Create `suggestion_bridge` module in `crosshook-core` with `map_protondb_to_catalog()` function | None         |
| 0.2  | Unit tests for mapping known env var pairs to catalog IDs                                       | 0.1          |

### Phase 1: Apply-to-Profile UI (3-4 tasks)

| Task | Description                                                                              | Dependencies |
| ---- | ---------------------------------------------------------------------------------------- | ------------ |
| 1.1  | Add `apply_protondb_suggestion` Tauri command                                            | Phase 0      |
| 1.2  | Extend `ProtonDbLookupCard.tsx` with "Enable optimization" toggle for mapped suggestions | 1.1          |
| 1.3  | Add "Copy to custom env vars" action for unmapped suggestions                            | 1.1          |
| 1.4  | Handle state synchronization between ProtonDB card and Launch tab optimizations          | 1.2          |

### Phase 2: Enhanced Aggregation (2-3 tasks)

| Task | Description                                                   | Dependencies    |
| ---- | ------------------------------------------------------------- | --------------- |
| 2.1  | Add Proton version weighting to `normalize_report_feed()`     | Phase 1 shipped |
| 2.2  | Add tier-weighted scoring (Platinum/Gold reports rank higher) | 2.1             |
| 2.3  | Add conflict detection for contradictory suggestions          | 2.2             |

### Phase 3: Suggestion Tracking (1-2 tasks, conditional)

| Task | Description                                                             | Dependencies             |
| ---- | ----------------------------------------------------------------------- | ------------------------ |
| 3.1  | Add `suggestion_actions` column/table for recording applied suggestions | Phase 1 adopted by users |
| 3.2  | Show "Previously applied" state in ProtonDB card                        | 3.1                      |

### Phase 4: ML Model (deferred indefinitely)

Not broken down. Prerequisites: Phases 0-2 shipped and generating data; heuristic approach demonstrably insufficient; additional maintainer bandwidth available.

---

## Key Decisions Needed

1. **Rename the feature?** "ML-assisted configuration" sets expectations for ML that is not justified. Recommend "Community-driven configuration suggestions" or "ProtonDB optimization suggestions" to reflect the actual implementation approach.

2. **Priority re-rating?** Given existing infrastructure, the remaining effort is Medium (not Very High). Should the priority be bumped from P3 to P2 given the reduced cost?

3. **Scope of Phase 0**: Should the catalog bridge only map to the 25 existing optimization entries, or should it also handle custom env vars that don't map to known entries?

4. **State synchronization approach**: When a suggestion is applied from the ProtonDB card, should it use Tauri events to update the Launch tab, or should both components share state through a lifted React context?

5. **Suggestion attribution**: Should applied optimizations carry metadata about their ProtonDB origin (e.g., "Suggested by 23 ProtonDB reports"), or should they be indistinguishable from manually-enabled optimizations?

---

## Open Questions

1. **ProtonDB API longevity**: The API is unofficial. Is there a community effort to formalize it? Should CrossHook cache reports more aggressively (24h+ TTL) as a hedge against API unavailability?

2. **Cross-game generalization**: Some optimizations (e.g., `PROTON_NO_ESYNC=1`) are game-specific workarounds, while others (e.g., `DXVK_ASYNC=1`) are broadly beneficial. Should the suggestion UI distinguish "this helps specifically for Game X" from "this helps most games"?

3. **ProtonDB report quality**: Reports vary wildly in quality. Some contain precise launch options; others contain "works fine" with no configuration data. The existing aggregation handles this via frequency weighting, but should there be a minimum report count threshold before showing suggestions?

4. **Interaction with community taps**: If a community tap already includes optimizations for a game, should ProtonDB suggestions be suppressed, shown alongside, or shown with lower priority?

5. **Feedback to ProtonDB**: Should CrossHook offer to submit reports back to ProtonDB based on launch success/failure? This is out of scope for this feature but would close the feedback loop.

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs` -- ProtonDB module entry point
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs` -- Report feed aggregation and env var extraction (the core of "ML-assisted" suggestions)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` -- ProtonDB API client with caching
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs` -- Data models including `ProtonDbEnvVarSuggestion`, `ProtonDbRecommendationGroup`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs` -- Optimization catalog (25 entries, target for suggestion mapping)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` -- `LaunchOptimizationsSection` (write target for applied suggestions)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts` -- Frontend hook for ProtonDB data
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProtonDbLookupCard.tsx` -- UI component to extend with apply actions
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs` -- Tauri IPC command for ProtonDB lookup
