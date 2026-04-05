# Feature Spec: Community-Driven Configuration Suggestions

## Executive Summary

This feature extends CrossHook's existing ProtonDB integration to surface actionable configuration suggestions during profile creation and editing. The existing `protondb` module already fetches community reports, extracts environment variables from freeform text, aggregates by frequency, and caches with TTL — the "ML-assisted" label is premature and should be renamed. The remaining work is primarily frontend wiring: adding an "Apply" flow to the existing `ProtonDbLookupCard`, mapping extracted env vars to known optimization catalog entries, and expanding the env var blocklist to prevent LD_PRELOAD-class injection. No new dependencies, no new tables (Phase 1), and no ML infrastructure are required. The original "Very High" effort rating is outdated; actual remaining effort is **Low-Medium**.

## External Dependencies

### APIs and Services

#### ProtonDB (Primary)

- **Documentation**: No official API. Community docs at [bdefore/protondb-data](https://github.com/bdefore/protondb-data), [Trsnaqe/protondb-community-api](https://github.com/Trsnaqe/protondb-community-api)
- **Authentication**: None required
- **Already Consumed Endpoints** (implemented in `protondb/client.rs`):
  - `GET /api/v1/reports/summaries/{appId}.json`: Tier/score/confidence
  - `GET /data/counts.json`: Report feed hash derivation
  - `GET /data/reports/all-devices/app/{hash}.json`: Individual reports with `launchOptions` and `concludingNotes`
- **Rate Limits**: No documented limits. CrossHook caches with 6h TTL (`CACHE_TTL_HOURS = 6`)
- **Pricing**: Free. ODbL license (share-alike on adapted databases; internal caching exempt)

#### PCGamingWiki (Supplemental, Future)

- **Documentation**: [pcgamingwiki.com/wiki/PCGamingWiki:API](https://www.pcgamingwiki.com/wiki/PCGamingWiki:API)
- **Authentication**: None required
- **Key Endpoint**: Cargo query API for structured game data (`?action=cargoquery&tables=Infobox_game`)
- **Value**: Structured Linux/Proton workaround data. Not needed for Phase 1.

### Libraries and SDKs

No new dependencies required. All existing:

| Library                | Already in Cargo.toml | Purpose                |
| ---------------------- | :-------------------: | ---------------------- |
| `reqwest` 0.12         |          Yes          | HTTP client (ProtonDB) |
| `serde` / `serde_json` |          Yes          | Serialization          |
| `rusqlite` 0.39        |          Yes          | SQLite cache           |
| `chrono`               |          Yes          | Timestamps             |

### External Documentation

- [ProtonDB Data Dumps](https://github.com/bdefore/protondb-data): ODbL-licensed report corpus
- [ODbL License](https://opendatacommons.org/licenses/odbl/): Cached data must not be exported in shareable artifacts

## Business Requirements

### User Stories

**Primary User: Linux gamer configuring Proton/Wine launch options**

- **US-1**: As a gamer setting up a new profile, I want CrossHook to automatically show me the most commonly used ProtonDB launch configurations for this game, so I don't have to search Reddit or ProtonDB myself.
- **US-2**: As a gamer, I want to see which suggestions map to known optimization toggles (WINED3D, ESYNC, FSYNC) so I can apply them with one click.
- **US-3**: As a gamer with existing env vars, I want conflict detection before overwriting my values, with per-key resolution.
- **US-4**: As a gamer offline, I want cached suggestions with a staleness indicator, never blocking profile creation.
- **US-5**: As a gamer, I want to see how many reports back each suggestion (e.g., "12 reports") to judge reliability.
- **US-6**: As a gamer who tried a suggestion that didn't help, I want to dismiss it for this session.

### Business Rules

1. **BR-1 — No auto-apply**: Suggestions MUST NEVER be applied without explicit user action. This is a security and correctness hard constraint.
2. **BR-2 — Safety filtering**: All env vars pass through `is_safe_env_key()`, `is_safe_env_value()`, and the expanded `RESERVED_ENV_KEYS` blocklist (including `LD_PRELOAD`, `PATH`, `HOME` — see Security). Re-validate at write time.
3. **BR-3 — Cache TTL**: ProtonDB responses cached in `external_cache_entries` with 6h TTL. Stale cache served when offline (`allow_expired = true`).
4. **BR-4 — Env var write target**: Accepted suggestions write to `profile.launch.custom_env_vars: BTreeMap<String, String>`. Catalog-matched suggestions toggle `launch.optimizations.enabled_option_ids`.
5. **BR-5 — Conflict detection**: When a suggested key already exists with a different value, show per-key resolution UI. Same-value matches silently skipped.
6. **BR-6 — Steam App ID required**: Suggestions only shown when `launchMethod in {steam_applaunch, proton_run}` and `steam.app_id` is set.
7. **BR-7 — ODbL compliance**: Cached ProtonDB data must never be included in profile exports or community tap distributions.
8. **BR-8 — Note-text extraction excluded**: `concluding_notes` is freeform prose where negation ("don't use X") could be misinterpreted. Only the structured `launch_options` field is parsed for env vars.
9. **BR-9 — Suggestion grouping**: Groups ranked by `supporting_report_count` (descending). Max 3 env groups, 3 launch groups, 4 note groups (existing constants).

### Edge Cases

| Scenario                                                    | Expected Behavior                                                           |
| ----------------------------------------------------------- | --------------------------------------------------------------------------- |
| No ProtonDB entry for game (404)                            | "No community data available" — no suggestions shown                        |
| Game exists but zero reports                                | Empty suggestion list with informational message                            |
| All reports use copy-only launch strings                    | Only copy-only groups shown; no env var apply actions                       |
| Report feed hash mismatch (404)                             | Retry once with refreshed counts; fallback to stale cache or degraded group |
| Env var matches reserved key (LD_PRELOAD)                   | Filtered out by `safe_env_var_suggestions()` — never shown                  |
| Suggested value matches existing value                      | Treated as `unchangedKeys` — silently skipped                               |
| Cache payload exceeds 512 KiB                               | `NULL payload_json` stored; aggregated payloads are typically < 5 KiB       |
| Optimization catalog entry removed after suggestion matched | Degrade to raw env-var apply mode                                           |
| Network unreachable, no cache exists                        | `unavailable` state — no suggestions, non-blocking notice                   |

### Success Criteria

- [ ] Top env var patterns from ProtonDB reports surfaced in profile creation/editing UI
- [ ] Catalog-matched suggestions shown as named optimization toggles
- [ ] No existing `custom_env_vars` value silently overwritten — conflicts always prompt
- [ ] Cached suggestions available offline with staleness indicator
- [ ] Every suggestion shows report count attribution
- [ ] `LD_PRELOAD`, `PATH`, `HOME`, and `LD_*` prefix blocked from suggestion output
- [ ] No code path writes to profile state without explicit user action
- [ ] No regression in existing ProtonDB lookup card behavior

## Technical Specifications

### Architecture Overview

```text
                      +-----------------------+
                      |    ProtonDB Servers    |
                      +-----------+-----------+
                                  |
                      [existing reqwest client, 6s timeout]
                                  |
                      +-----------v-----------+
                      |  protondb/client.rs   |  (existing)
                      +-----------+-----------+
                                  |
                      +-----------v-----------+
                      | protondb/aggregation  |  (existing)
                      | .rs - env var extract |
                      +-----------+-----------+
                                  |
                 +----------------v-----------------+
                 |     protondb/suggestions.rs      |  (NEW)
                 | derive_suggestions(lookup,       |
                 |   profile, dismissed) -> Set     |
                 +----------------+-----------------+
                                  |
          +-----------------------+------------------------+
          |                                                |
+---------v-----------+                      +-------------v-----------+
| MetadataStore       |                      | commands/protondb.rs    |
| external_cache_     |                      | (extend with 3 cmds)   |
| entries (existing)  |                      +-------------+-----------+
+---------------------+                                    |
                                               [Serde JSON over IPC]
                                                           |
                                             +-------------v-----------+
                                             | useProtonDbSuggestions  |
                                             | .ts (NEW hook)         |
                                             +-------------------------+
```

### Data Models

#### `ProtonDbSuggestionSet` (new, runtime-only)

| Field                       | Type                              | Description                           |
| --------------------------- | --------------------------------- | ------------------------------------- |
| `app_id`                    | `String`                          | Steam App ID                          |
| `profile_name`              | `String`                          | Profile this set was computed against |
| `env_var_suggestions`       | `Vec<EnvVarSuggestionItem>`       | Ranked suggestions with status        |
| `launch_option_suggestions` | `Vec<LaunchOptionSuggestionItem>` | Copy-only strings                     |
| `advisory_notes`            | `Vec<ProtonDbAdvisoryNote>`       | Community notes                       |
| `cache`                     | `Option<ProtonDbCacheState>`      | Freshness metadata                    |
| `actionable_count`          | `u32`                             | Count of `New` status suggestions     |

#### `EnvVarSuggestionItem` (new, runtime-only)

| Field           | Type                       | Description                                         |
| --------------- | -------------------------- | --------------------------------------------------- |
| `suggestion`    | `ProtonDbEnvVarSuggestion` | Existing type (key, value, source_label, count)     |
| `status`        | `SuggestionStatus`         | `New` / `AlreadyApplied` / `Conflict` / `Dismissed` |
| `current_value` | `Option<String>`           | Profile's current value when status is `Conflict`   |

#### `SuggestionStatus` (new enum)

`New` | `AlreadyApplied` | `Conflict` | `Dismissed`

**One new SQLite table for Phase 1**: `suggestion_dismissals` (schema v17) for persistent dismissed-suggestion tracking with 30-day auto-expiry. The existing `external_cache_entries` table handles all ProtonDB response caching.

```sql
-- Schema v17
CREATE TABLE IF NOT EXISTS suggestion_dismissals (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id   TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    app_id       TEXT NOT NULL,
    suggestion_key TEXT NOT NULL,  -- e.g., "env:PROTON_USE_WINED3D=1"
    dismissed_at TEXT NOT NULL,    -- RFC 3339 timestamp
    expires_at   TEXT NOT NULL     -- dismissed_at + 30 days
);
CREATE INDEX IF NOT EXISTS idx_suggestion_dismissals_profile_app
    ON suggestion_dismissals(profile_id, app_id);
```

### API Design

#### `protondb_get_suggestions` (new Tauri command)

**Purpose**: Derive suggestions for a game profile from cached/fresh ProtonDB data.

```typescript
// Frontend
const result = await invoke<ProtonDbSuggestionSet>('protondb_get_suggestions', {
  appId: '1245620',
  profileName: 'my-game.toml',
  forceRefresh: false,
});
```

Internally calls `lookup_protondb()` (handles caching), then `derive_suggestions()` to compare against profile state.

#### `protondb_accept_suggestion` (new Tauri command)

**Purpose**: Accept a single env var suggestion into a profile's `custom_env_vars`.

**Validation (at write time, not trusted from cache)**:

1. Key passes `is_safe_env_key()`
2. Value passes `is_safe_env_value()`
3. Key not in expanded `RESERVED_ENV_KEYS` (including `LD_*` prefix)
4. Profile exists and is loadable

**Write path**: Load profile -> insert into `custom_env_vars` -> save via `ProfileStore::save()` -> record config revision.

#### `protondb_dismiss_suggestion` (new Tauri command)

**Purpose**: Dismiss a suggestion persistently. Writes to `suggestion_dismissals` table (schema v17) with 30-day auto-expiry. Expired dismissals are evicted on read.

### System Integration

#### Files to Create (2)

- `crates/crosshook-core/src/protondb/suggestions.rs`: Suggestion engine — `derive_suggestions()`, status comparison, env var key allowlist expansion
- `src/hooks/useProtonDbSuggestions.ts`: Frontend hook wrapping suggestion IPC

#### Files to Modify (5)

- `crates/crosshook-core/src/protondb/mod.rs`: Add `pub mod suggestions;`, re-export types
- `crates/crosshook-core/src/protondb/aggregation.rs`: Make `is_safe_env_key()` / `is_safe_env_value()` `pub(crate)`; expand `RESERVED_ENV_KEYS` with `LD_PRELOAD`, `PATH`, `HOME`, etc.
- `src-tauri/src/commands/protondb.rs`: Add 3 new commands
- `src-tauri/src/lib.rs`: Register new commands in `invoke_handler`
- `src/types/protondb.ts`: Add TypeScript interfaces for suggestion types

## UX Considerations

### User Workflows

#### Primary Workflow: Suggestions During Profile Creation/Editing

1. User opens profile (creation or edit) with Steam/Proton launch method
2. `ProtonDbLookupCard` mounts, calls `useProtonDbLookup(appId)`
3. Panel shows tier badge, recommendations with env var groups
4. Each env var group has "Apply Suggested Env Vars" button
5. User clicks Apply -> `mergeProtonDbEnvVarGroup()` checks for conflicts
6. No conflicts: applied immediately, status message shown
7. Conflicts: `ProtonDbOverwriteConfirmation` dialog with per-key "Keep current" / "Use suggestion"
8. Applied env vars appear in `launch.custom_env_vars` table
9. Catalog-matched suggestions show "Enable [Optimization Name]" toggle instead

#### Error Recovery Workflow

1. Live fetch fails (network/404/hash mismatch)
2. Backend attempts stale cache (`allow_expired = true`)
3. Stale cache found: amber banner "Showing cached ProtonDB guidance (from X hours ago)"
4. No cache: "ProtonDB is unavailable" banner, rest of editor usable
5. Retry via "Refresh" button

### UI Patterns

| Component           | Pattern                                          | Notes                                                 |
| ------------------- | ------------------------------------------------ | ----------------------------------------------------- |
| Suggestion panel    | Collapsible inline panel (not modal)             | Co-located with form fields; never blocks interaction |
| Confidence display  | Three-tier badges (High/Medium/Low)              | Green/Amber/Gray; no red for low confidence           |
| Source attribution  | "Based on N ProtonDB reports" chip               | Mandatory on every suggestion; link to ProtonDB page  |
| Conflict resolution | Existing `ProtonDbOverwriteConfirmation` dialog  | Per-key independent choices                           |
| Dismissal           | Per-suggestion X button with brief undo          | Session-only in Phase 1                               |
| Catalog match       | "Enable [Name]" toggle distinct from raw "Apply" | Different visual treatment to signal named feature    |

### Accessibility Requirements

- Collapsed panel does not interfere with form tab order (`tabindex="-1"` on panel items when collapsed)
- Max 5-6 suggestions visible at once; "Show N more" for overflow
- Error messages in plain language, no internal technical details exposed
- Loading state uses skeleton loader, not full-page overlay

### Performance UX

- **Loading States**: Skeleton loader in panel header while fetching; form remains usable
- **Stale-While-Revalidate**: Serve cache immediately, background refresh, "Updated" indicator on change
- **Timeout**: 6-second timeout (matches existing `client.rs`); show error/retry state, not infinite spinner
- **Suggestion derivation**: O(n\*m) where n = suggestions (max ~9) and m = profile env vars (typically <20) — near-instant

## Recommendations

### Implementation Approach

**Recommended Strategy**: Incremental enhancement of existing infrastructure, not a new ML system.

**Phasing:**

1. **Phase 0 — Security + Catalog Bridge (Low effort)**
   - Expand `RESERVED_ENV_KEYS` with `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `PATH`, `HOME`, `SHELL` + `LD_*` prefix block
   - Create env var -> optimization catalog mapping function (~100-200 lines Rust)
   - Add golden test fixtures for extraction validation
   - **Mandatory**: Security allowlist is a hard prerequisite for any "apply" flow

2. **Phase 1 — Apply-to-Profile UI (Medium effort)**
   - Create `suggestions.rs` with `derive_suggestions()` comparing ProtonDB data against profile state
   - Add 3 Tauri commands (`protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion`)
   - Create `useProtonDbSuggestions.ts` hook
   - Extend `ProtonDbLookupCard` with Apply/Enable actions
   - Wire existing `mergeProtonDbEnvVarGroup` and `ProtonDbOverwriteConfirmation` into creation wizard

3. **Phase 2 — Enhanced Aggregation (Medium effort, deferred)**
   - Proton version weighting (prefer suggestions from recent versions)
   - Tier-weighted scoring (Platinum/Gold reports rank higher)
   - Contradiction detection between suggestions

4. **Phase 3+ — Feedback Tracking and ML (deferred indefinitely)**
   - `suggestion_feedback` SQLite table for accepted/dismissed tracking
   - ML model only if heuristic approach demonstrably fails and maintainer bandwidth allows

### Technology Decisions

| Decision                                  | Recommendation                                   | Rationale                                                                                    |
| ----------------------------------------- | ------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| ML vs. Heuristic                          | Heuristic (existing regex extraction)            | ML unjustified for P3 feature; regex covers 80%+ of ProtonDB patterns                        |
| Separate suggestion cache                 | No — derive on-demand from cached lookup         | Avoids staleness bugs when profile changes                                                   |
| New SQLite tables (Phase 1)               | Yes — `suggestion_dismissals` table (schema v17) | Persistent dismissals survive app restart; 30-day auto-expiry prevents permanent suppression |
| Dedicated accept command vs. generic save | Dedicated command                                | Enables re-validation at write time, config revision provenance                              |
| Feature name                              | "Community-driven configuration suggestions"     | Accurately reflects heuristic approach, avoids ML expectation                                |

### Quick Wins

- **Phase 0 security fix**: Expanding `RESERVED_ENV_KEYS` is independent of the UI work and should ship immediately as a hardening fix to the existing ProtonDB module
- **Report count visibility**: Already implemented — `supporting_report_count` flows to frontend

### Future Enhancements

- **PCGamingWiki integration**: Structured data source that could feed the same catalog bridge
- **Community-curated configs**: Per-game TOML presets in community taps (complementary, not replacement)
- **Proton version recommendations**: Surfacing which Proton versions work best (data exists in reports)

## Risk Assessment

### Technical Risks

| Risk                                     | Likelihood | Impact | Mitigation                                                     |
| ---------------------------------------- | ---------- | ------ | -------------------------------------------------------------- |
| ProtonDB API changes/breaks              | Medium     | High   | Existing retry + stale cache fallback; add 404 rate monitoring |
| Extracted suggestions break games        | Low        | High   | Never auto-apply; report count attribution; safety validation  |
| Mapping gaps (ProtonDB vars vs. catalog) | High       | Low    | Expected; unmapped vars shown as raw "Copy to custom env vars" |
| ProtonDB rate-limits CrossHook           | Medium     | Medium | 6h cache TTL limits frequency; add backoff for 429s            |

### Integration Challenges

- **Profile mutation from ProtonDB card**: New coupling between display and save path — mitigated by dedicated Tauri command with re-validation
- **State synchronization**: Applied suggestion must reflect in both ProtonDB card and Launch tab — use Tauri events or lifted React state

### Security Considerations

#### Critical — Hard Stops

| Finding                                                              | Risk                                                                      | Required Mitigation                                                                                         |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| S2: `RESERVED_ENV_KEYS` missing `LD_PRELOAD`, `PATH`, `HOME`, `LD_*` | Code execution via library injection if user accepts malicious suggestion | Expand blocklist + add `LD_*` prefix block. Re-validate at write time. **Must ship before any apply flow.** |

#### Warnings — Must Address

| Finding                                           | Risk                                                | Mitigation                                                                                        | Alternatives                                                          |
| ------------------------------------------------- | --------------------------------------------------- | ------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| S1: No positive name allowlist for env keys       | Unknown vars could have side effects                | Fix S2 first (reduces surface); long-term add positive allowlist                                  | Pattern-based approach with expanded blocklist is pragmatically sound |
| S4: Copy-only launch strings surfaced unsanitized | Shell metacharacters in copy-only strings           | Confirm copy-only strings are display-only, never auto-applied; label "for manual Steam use only" | Light sanitization pass (strip null, truncate)                        |
| S5: XSS via report text in React                  | Attacker-controlled text executes in WebView        | Audit React components for `dangerouslySetInnerHTML`; use plain-text interpolation                | Add HTML entity stripping in `normalize_text`                         |
| S6: No HTTP response size limit                   | Unbounded memory allocation from malformed response | Add `MAX_RESPONSE_BYTES` (1 MB) guard before `.json()`                                            | Use `bytes_stream()` with byte counter                                |
| S8: reqwest/hyper RUSTSEC-2024-0042               | Known vulnerability in transitive dependency        | Run `cargo tree -p hyper` to verify; add `cargo audit` to CI                                      | Pin hyper >= 0.14.26 or reqwest >= 0.12                               |

#### Advisories — Best Practices

- S7: Cache TTL (6h) acceptable; stale-on-failure fallback is correct
- S9: Add `cargo audit` to CI for ongoing dependency hygiene (not feature-specific)

## Task Breakdown Preview

### Phase 0: Security + Catalog Bridge

**Focus**: Mandatory security hardening + env var -> catalog mapping
**Tasks**:

- Expand `RESERVED_ENV_KEYS` with LD*PRELOAD family + `LD*\*` prefix block
- Add test cases for blocked keys (`ld_preload_is_rejected_as_env_suggestion`)
- Create catalog bridge function mapping env var pairs to optimization catalog IDs
- Unit tests for catalog mapping
  **Parallelization**: Security fix and catalog bridge can run concurrently

### Phase 1: Apply-to-Profile UI + Dismissal Persistence

**Focus**: Actionable suggestion flow from ProtonDB data to profile fields; persistent dismissals
**Dependencies**: Phase 0 (security blocklist must be in place)
**Tasks**:

- Schema v17 migration: create `suggestion_dismissals` table with `ON DELETE CASCADE` and 30-day expiry
- Create `protondb/suggestions.rs` with `derive_suggestions()` (reads dismissals from MetadataStore)
- Add 3 Tauri commands to `commands/protondb.rs` (`protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion`)
- `protondb_dismiss_suggestion` writes to `suggestion_dismissals` table; evict expired rows on read
- Create `useProtonDbSuggestions.ts` hook
- Extend `ProtonDbLookupCard` with Apply/Enable/Dismiss actions
- Wire suggestion flow into profile creation wizard
- Add TypeScript interfaces for suggestion types
  **Parallelization**: Backend (suggestions.rs + commands + migration) and frontend (hook + UI) can proceed in parallel after interfaces are agreed

### Phase 2: Enhanced Aggregation (deferred)

**Focus**: Improve suggestion quality via smarter ranking
**Dependencies**: Phase 1 shipped and validated
**Tasks**:

- Proton version weighting in `normalize_report_feed()`
- Tier-weighted scoring (Platinum/Gold rank higher)
- Contradiction detection for conflicting suggestions

## Decisions (Resolved)

1. **Feature rename**: **Yes** — Rename to "Community-driven configuration suggestions". ML is not justified; the heuristic pipeline delivers the value.

2. **Priority re-rating**: **Yes** — Bumped from P3 to P2. Remaining effort is Low-Medium given existing infrastructure.

3. **Dismissed suggestion persistence**: **SQLite table from day 1** — `suggestion_dismissals` table (schema v17) with `ON DELETE CASCADE` on `profile_id` and 30-day auto-expiry. Dismissed suggestions survive app restart and are cleaned up when profiles are deleted.

4. **Suggestion attribution**: **Deferred** — Applied suggestions become normal profile fields. No provenance tracking at this time.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): ProtonDB API endpoints, PCGamingWiki, Steam API, Rust crates
- [research-business.md](./research-business.md): User stories, business rules, domain model, workflows
- [research-technical.md](./research-technical.md): Architecture, data models, Tauri commands, text extraction pipeline
- [research-ux.md](./research-ux.md): Workflow design, competitive analysis (ProtonDB, Lutris, Bottles, Steam Deck, Heroic)
- [research-security.md](./research-security.md): Severity-leveled findings (S1-S9), LD_PRELOAD risk, env var validation
- [research-practices.md](./research-practices.md): Existing reusable code inventory, KISS assessment, build-vs-depend
- [research-recommendations.md](./research-recommendations.md): Phasing strategy, risk assessment, alternative approaches
