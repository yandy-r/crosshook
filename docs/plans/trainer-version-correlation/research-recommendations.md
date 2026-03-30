# Trainer-Version Correlation: Recommendations & Risk Assessment

## Executive Summary

Trainer-version correlation for CrossHook can be built primarily by extending existing infrastructure — the VDF parser, metadata SQLite layer, health scoring pipeline, and community profile schema — rather than introducing new architectural patterns. The metadata database (added after issue #41 was written) transforms this from a simple "check and warn" feature into a full version history and compatibility intelligence system.

**Cross-team consensus** from all seven research perspectives:

- **Zero new crates required** — all dependencies (`rusqlite`, `sha2`, `chrono`, `serde`, `uuid`) are already in `Cargo.toml` (practices)
- **No CRITICAL security findings** — three WARNINGs (unbounded version strings, pinned_commit validation, community data trust boundary) are all targeted fixes (security)
- **SQLite-only storage is correct** — no TOML profile format changes needed; the metadata DB handles this cleanly (tech-design, practices)
- **Trainer version is fundamentally user-asserted** — no reliable auto-detection exists; supplement with SHA-256 file hash for automated change detection (business, tech-design, API)
- **"Untracked is not broken"** — first-time profiles must show "no data" not "mismatch" to avoid false positive trust erosion (business)
- **Community data is display-only** — community version/compatibility data must NEVER drive behavioral outcomes (warnings, launch gates); only local + Steam data controls behavior (security W3)
- **DB failure must not block launch** — all version DB calls must be wrapped in availability guards; version check is informational only (security A8)
- **On-demand checks, not pre-launch gates** — version checks happen during startup reconciliation and health dashboard scans, not in the synchronous launch path, to avoid Steam Deck SD card latency (practices)
- **"Launch Anyway" must always be the primary action** — never block gameplay for version mismatches; WeMod's success confirms users demand this (UX)

**Recommended approach**: Phased rollout starting with Steam build ID extraction and on-demand mismatch detection via the health system (Phase 1+2 as MVP, ~9 days), with community integration and analytics as follow-on phases.

**Key insight**: The codebase already parses Steam appmanifest ACF files via `steam/manifest.rs` using the VDF parser (`steam/vdf.rs`), but currently only extracts `appid` and `installdir`. Adding `buildid` extraction is a ~10-line change — one `get_child("buildid")` call — that unlocks the entire feature's foundation.

---

## Implementation Recommendations

### Technical Approach

#### 1. Steam Build ID Extraction (Foundation)

**Current state**: `manifest.rs:parse_manifest()` reads `appid` and `installdir` from VDF `AppState` nodes. The `buildid` field exists in every Steam appmanifest ACF file but is not extracted.

**Recommendation**: Extend `parse_manifest()` to also extract `buildid` and `LastUpdated`. The VDF parser already supports case-insensitive key lookup via `get_child()`, so this requires zero parser changes — just one additional `get_child("buildid")` call in the existing function (practices research confirmed this).

```
appmanifest_12345.acf → AppState → buildid: "15432876"
                                  → LastUpdated: "1711234567"
                                  → StateFlags: "4"
```

**Integration point**: The `SteamGameMatch` struct (`steam/models.rs`) should gain a `build_id: Option<String>` field, populated during `find_game_match()` in `manifest.rs`.

**Security note (A1)**: Validate that `buildid` is numeric-only before storage — Steam always emits a decimal integer, but the VDF parser accepts any string. Apply `.trim()` and verify all chars are ASCII digits.

#### 2. Version Snapshot Storage (SQLite Migration 8→9)

**Recommendation**: Add a `version_correlation` table following the existing migration ladder pattern in `migrations.rs`. The table name `version_correlation` (rather than `version_snapshots`) aligns with the feature name and avoids confusion with health snapshots.

**Consensus schema** (synthesized from tech-design and practices research):

```sql
CREATE TABLE version_correlation (
    profile_id           TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    game_build_id        TEXT,
    game_manifest_path   TEXT,
    trainer_version_hint TEXT,
    trainer_file_hash    TEXT,
    verification_method  TEXT NOT NULL DEFAULT 'launch_success',
    is_known_good        INTEGER NOT NULL DEFAULT 1,
    verified_at          TEXT NOT NULL,
    PRIMARY KEY (profile_id)
);
CREATE INDEX idx_version_correlation_verified_at ON version_correlation(verified_at);

ALTER TABLE launch_operations ADD COLUMN game_build_id TEXT;
```

**Design rationale**:

- Single row per profile (latest snapshot), not a history table — same pattern as `health_snapshots` (practices confirmed this is the right model)
- `game_build_id`: opaque string from Steam manifest, validated as numeric before storage (security A1)
- `trainer_version_hint`: user-asserted or community-sourced string (business analysis confirmed trainers don't expose version metadata programmatically)
- `trainer_file_hash`: SHA-256 of trainer executable — `sha2` crate already in `Cargo.toml` via `profile_sync.rs` (practices reuse opportunity). Provides automated change detection even when user doesn't set a version string
- `verification_method`: enum of `launch_success`, `user_confirmed`, `community_import` — indicates confidence level (tech-design recommendation)
- `is_known_good`: flag to support future compatibility matrix without schema change (tech-design: "start with simple comparison, design schema for matrix")
- `game_build_id` on `launch_operations`: enables historical queries ("when did buildid change?") without a separate history table
- ON DELETE CASCADE follows the `health_snapshots` pattern

**New file**: `metadata/version_store.rs` — copy the upsert/load/lookup triad from `health_store.rs` verbatim (practices: highest-confidence reuse target).

#### 3. Mismatch Detection via Health System

**Recommendation**: Integrate version mismatch as a new `HealthIssue` type rather than building a parallel alerting system.

**Why**: The health scoring pipeline (`health_store.rs`, `HealthSnapshotRow`, `ProfileHealthReport` in TypeScript) already has:

- Per-profile issue tracking with severity levels (error/warning/info)
- Persistent snapshots in SQLite
- Frontend rendering via `HealthDashboardPage.tsx` and `HealthBadge.tsx`
- Enrichment pattern in `commands/health.rs` that can add `version_correlation` as optional field (practices)
- Sortable metadata table

A version mismatch would be a `warning`-severity health issue:

```
field: "game.build_id"
message: "Game has been updated since last verified launch (build 15432876 → 15498234)"
remediation: "Launch the game to verify trainer compatibility, or update the trainer"
severity: "warning"
```

**Three-layer warning pattern** (UX research — competitive analysis of WeMod, Vortex, Heroic):

| Layer                                 | Component                                           | Trigger                                       | User Action                                              |
| ------------------------------------- | --------------------------------------------------- | --------------------------------------------- | -------------------------------------------------------- |
| **1. Indicator badge** (passive)      | `HealthBadge` on profile card/sidebar               | Version mismatch detected during startup scan | None — visual awareness only                             |
| **2. Actionable banner** (persistent) | Inline banner on LaunchPage when profile selected   | Profile has active mismatch                   | "Launch Anyway" (primary CTA) / "Mark as Verified"       |
| **3. Post-launch prompt** (one-time)  | Inline prompt after successful launch with mismatch | Launch completed for mismatched profile       | "Trainer Worked" → auto-verify / "Had Issues" → escalate |

**Critical UX constraints** (from competitive analysis + security revision 3):

- **"Launch Anyway" must always be the primary action** — WeMod's success is built on never blocking gameplay. Vortex's generic "incompatible" warnings with no override are widely hated.
- **Never use modals for version warnings** — "cry wolf" effect kills feature utility
- **Show specific version numbers but NOT filesystem paths** — Vortex's vague "incompatible mod" warnings with no detail are the anti-pattern to avoid. However, mismatch warnings must use semantic category names (e.g., "build 15432876 → 15498234"), never absolute filesystem paths like `/home/user/.config/...`. This follows the existing health system pattern (security revision 3).
- **Community data gets a disclaimer** — when showing community-sourced compatibility, add "community data, may not reflect your configuration" (Heroic pattern)
- **Post-launch prompt responses are local-only** — "Did trainer work?" Y/N responses are stored only in local metadata (`version_correlation`), never transmitted to community taps or external services (security revision 3)

**"Untracked is not broken" rule** (business): Profiles with no version baseline (`version_correlation` row absent) should show `status: "untracked"` — never `"mismatch"`. This is critical for avoiding false positive trust erosion on new/imported profiles.

**Community data trust boundary** (security W3): Community version/compatibility data from taps is **display-only**. It must NEVER suppress a locally-detected mismatch warning or trigger a false positive. Only local Steam manifest data + local launch history drive behavioral outcomes. This is a hard architectural constraint that must be enforced before implementation begins.

#### 4. Version Check Flow (On-Demand, Not Pre-Launch)

**Recommendation**: On-demand via startup reconciliation and health dashboard — NOT in the synchronous launch path.

**Why not pre-launch?** (practices research, confirmed): Pre-launch manifest reads add measurable latency on Steam Deck with slow SD card storage. The codebase consistently uses on-demand patterns (health dashboard, auto-populate). Inserting a synchronous check into the launch hot path breaks this pattern and risks degrading the launch experience on the primary target device.

**Primary check flow — startup reconciliation**:

1. App starts → `startup.rs` runs existing profile sync
2. After sync, scan Steam manifests for all Steam-backed profiles (batch by `app_id` to deduplicate)
3. Compare current `buildid` against `version_correlation.game_build_id` for each profile
4. Update health badges asynchronously — mismatch profiles show warning badge in sidebar/profile list
5. User sees badges before ever clicking Launch

**Secondary check flow — health dashboard on-demand**:

1. User opens Health Dashboard → triggers full health recalculation (existing pattern)
2. Version correlation check runs as part of health scoring
3. Mismatch appears as `HealthIssue` with `warning` severity
4. "Bulk Check" button re-scans all manifests

**Launch-time behavior — record, don't gate**:

1. User clicks Launch → game launches immediately (no pre-check delay)
2. On successful launch completion (`record_launch_finished()`), read the current `buildid` from manifest
3. Update both `version_correlation` and `launch_operations.game_build_id`
4. If the user had a mismatch badge and the launch succeeded, the badge clears automatically

**Post-launch verification** (UX research: WeMod pattern):

1. After a successful launch with a mismatch, optionally prompt: "Did the trainer work correctly?"
2. If user confirms → `verification_method: user_confirmed`, clears mismatch
3. If user reports issue → mark `is_known_good: 0`, escalate severity to `error`

**TOCTOU mitigation** (tech-design, security A4): Record the buildid AT launch completion, not before. The manifest could change between check and launch, but this is negligible for a local desktop context.

**DB failure guard** (security A8): All version DB calls must be wrapped in `is_available()` guards. If the metadata store is unavailable, version checks silently degrade — launch must never be blocked by DB errors.

#### 5. Trainer Version Detection

**Recommendation**: User-supplied version string with file hash for automated change detection — phased from simple to advanced (synthesized from tech-design, practices, and API research).

**MVP (v1) — buildid + trainer file hash**:

1. **SHA-256 file hash** (automated): Hash the trainer executable on each successful launch. If hash changes since last verified launch, the trainer has been updated — even if the user hasn't set a version string. Uses existing `sha2` crate from `profile_sync.rs`. Hash stored as TEXT in `version_correlation.trainer_file_hash`, not exposed in UI (security).
2. **User-entered version string** (manual, authoritative): `trainer_version_hint` field in profile metadata. Takes precedence over hash for display.

**v1.5 — add trainer mtime tracking** (API researcher: Approach B): 3. **Trainer file `mtime`** (automated): Record trainer `.exe`'s modification time alongside hash. If trainer changed (`mtime` differs) but game `buildid` didn't, show "trainer was updated — you may want to re-verify." Note: `mtime` is less reliable than hash (can be reset by copy operations) but provides an additional signal.

**v2+ — filename heuristic** (API researcher: Approach C deferred): 4. **Filename heuristic** (automated, best-effort): Parse version-like patterns from trainer filenames (e.g., `EldenRing_v1.12_Trainer.exe` → `v1.12`). Auto-suggest but never auto-commit. PE version resource extraction (`pelite` crate) deferred indefinitely unless user demand justifies the new dependency — many trainers are packed/protected and lack VERSIONINFO.

**Business constraint**: Community authors may not increment `trainer_version` strings when releasing silent updates. The file hash covers this gap — hash change + same version string = "trainer updated, version not bumped."

### Phasing Strategy

#### Phase 1: Detection Foundation (Core Rust) — ~5 days

- Extend VDF manifest parsing to extract `buildid` (one `get_child` call)
- Add `SteamGameMatch.build_id` field
- Migration 8→9: `version_correlation` table + `launch_operations.game_build_id` column
- `metadata/version_store.rs`: CRUD functions mirroring `health_store.rs`
- `compute_correlation_status()` as a pure function for testability (practices)
- Unit tests for buildid extraction and snapshot CRUD
- Security fixes: A6 bounds for version fields in `community_index.rs`, buildid numeric validation

#### Phase 2: Integration (Tauri + Frontend) — ~4 days

- Startup reconciliation scan in `startup.rs` (batch manifest reads, async badge update)
- Post-launch version recording in `record_launch_finished()` flow
- Health system integration: version mismatch as `HealthIssue`
- Three-layer warning: health badge + LaunchPage banner + post-launch prompt
- TypeScript types in `src/types/version.ts`
- DB failure guard: wrap all version calls in `is_available()` (security A8)

#### Phase 3: User Experience — ~4 days

- Version info display on Launch page and Profile page
- "Mark as Verified" user action (explicit confirmation → `verification_method: user_confirmed`)
- "Did trainer work?" post-launch prompt (UX: WeMod pattern)
- Post-update re-check hook (after `update_game()` completes)
- Trainer version hint field in profile editor
- Community data disclaimer on community-sourced version info (UX: Heroic pattern)

#### Phase 3.5: Trainer mtime tracking (API: Approach B) — ~1 day

- Record trainer file `mtime` alongside SHA-256 hash in `version_correlation`
- Detect trainer-side changes independently from game buildid changes

#### Phase 4: Community & Analytics — ~5 days

- Community profile version data enrichment (display-only — W3 constraint)
- Version history queries from launch_operations
- Compatibility scoring based on version pair success rates (tiered badges aligned with existing `CompatibilityRating` enum — UX: ProtonDB vocabulary)
- Community tap version advisory integration (requires `CommunityProfileManifest` schema v2)
- Row-count pruning for launch_operations (security A7: 50 most recent per profile)

**Firm P1 scope** (business recommendation): Phase 1+2 as MVP. Snapshot on success, warn on mismatch via health system, community metadata as display-only. Community sharing is Phase 4 — resist scope inflation.

### Quick Wins

1. **Buildid extraction** (~10 lines of Rust): Modify `parse_manifest()` to also return buildid. Immediate value: can display current game build in UI.
2. **A6 bounds fix** (security W1, ~4 lines): Add `MAX_VERSION_BYTES = 256` check for `game_version` and `trainer_version` in `check_a6_bounds()` in `community_index.rs`. This is a standalone security improvement.
3. **Community metadata display**: The `CommunityProfileMetadata` already has `game_version` and `trainer_version` fields stored in the DB. Surface these more prominently in `CommunityBrowser.tsx` as compatibility hints.

---

## Improvement Ideas

### Related Features

1. **Auto-Trainer-Update Detection**: Monitor trainer file SHA-256 hash alongside game buildid. If trainer hash hasn't changed but game has updated, that's a stronger mismatch signal than buildid change alone. The `sha2` crate is already a dependency.

2. **Compatibility Scoring**: After Phase 4, compute a compatibility confidence score per profile based on:
   - Number of successful launches at current version pair
   - Community reports for this game+trainer+version combo
   - Time since last verification
     This could feed into the existing `CompatibilityRating` enum (Unknown/Broken/Partial/Working/Platinum).

3. **Version Pinning**: Allow users to "pin" a known-good version pair (`is_known_good` flag already in schema). If the game updates past the pinned version, show a stronger warning. Useful for users who deliberately stay on older game versions for trainer compatibility.

4. **Proton Version Correlation**: Track Proton version alongside game/trainer versions. Proton updates can also break trainer compatibility. The `ProtonInstall` struct already has version info.

5. **Bulk Version Check**: A "Check All" button on the Health Dashboard that scans all Steam-backed profiles for buildid changes in one pass. Efficient because it can batch-read all relevant appmanifest files (deduplicate by `app_id`).

### Future Enhancements

1. **SteamDB/PCGamingWiki Integration**: Use the `external_cache_entries` table to cache version-to-changelog mappings from public APIs. Show "Game updated to version X.Y.Z" alongside the buildid change notification. API researcher findings should inform feasibility.

2. **Launch History Version Timeline**: Visualize which game builds were used across launch history. The `launch_operations` table already tracks per-launch data; adding `game_build_id` creates a version timeline.

3. **Community Version Advisory Feed**: Community taps could publish version compatibility advisories ("FLiNG Elden Ring trainer v2 is broken with build 15498234"). Requires extending `CommunityProfileManifest` to schema version 2 with `known_good_builds: Vec<BuildRecord>` (business analysis).

### Optimization Opportunities

1. **Manifest caching**: Cache parsed manifest data in memory during a session to avoid re-reading ACF files on every health check. The manifest rarely changes within a single app session.

2. **Selective manifest scanning**: Only re-parse manifests for profiles that have been launched recently (use `launch_operations.started_at` to filter).

3. **Batch health recalculation**: When multiple profiles share a Steam library, batch-scan all manifests in that library at once instead of per-profile.

---

## Risk Assessment

### Technical Risks

| Risk                                                                     | Likelihood | Impact | Mitigation                                                                                                                                       |
| ------------------------------------------------------------------------ | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Steam manifest format changes (Valve updates ACF schema)                 | Low        | High   | VDF parser is schema-agnostic; buildid extraction uses simple key lookup. If field is missing, gracefully return None.                           |
| Trainer version inconsistency (no standard format)                       | High       | Medium | Treat trainer versions as opaque strings. Never parse or compare semantically. Supplement with SHA-256 file hash for automated change detection. |
| False positive mismatch alerts (buildid changes without gameplay impact) | Medium     | Medium | Default to `warning` severity, show actual build IDs so users can self-assess (business). "Untracked" distinct from "mismatch."                  |
| Steam beta branch buildid ambiguity                                      | Medium     | Low    | Show actual build IDs in warnings — users on beta branches know to expect this (business). Don't attempt branch detection.                       |
| Manifest read errors during launch (file locked by Steam)                | Medium     | Low    | Steam doesn't exclusively lock ACF files on Linux. Use non-blocking read with timeout fallback.                                                  |
| Database migration failure on existing installs                          | Low        | High   | Follow existing migration pattern with idempotent IF NOT EXISTS. Test upgrade from every prior schema version.                                   |
| Performance impact of manifest parsing at launch time                    | Low        | Low    | VDF parsing is ~1ms for typical manifests. The bottleneck is filesystem I/O, not parsing.                                                        |
| Trainer version string entropy (silent updates without version bump)     | High       | Low    | SHA-256 file hash catches this automatically even when version string doesn't change.                                                            |

### Security Findings to Address (Pre-Implementation)

| ID  | Severity | Finding                                                            | Fix                                                                                                          |
| --- | -------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| W1  | WARNING  | `game_version`/`trainer_version` unbounded in `check_a6_bounds()`  | Add `MAX_VERSION_BYTES = 256` check (~4 lines in `community_index.rs`)                                       |
| W2  | WARNING  | `pinned_commit` not validated before git subprocess in `taps.rs`   | Validate hex-only, 7-64 chars before passing to git                                                          |
| W3  | WARNING  | Community version data must not control behavioral outcomes        | Hard architectural constraint: community data is display-only; only local + Steam data drives warnings/gates |
| A1  | ADVISORY | Steam `buildid` should be validated as numeric-only before storage | Validate all chars are ASCII digits after `.trim()`                                                          |
| A5  | ADVISORY | Version comparison must not panic on malformed input               | Use `Result`-returning parse, fall back to string equality                                                   |
| A6  | ADVISORY | Normalize whitespace (`.trim()`) on both sides before comparing    | Apply `.trim()` in `compute_correlation_status()`                                                            |
| A7  | ADVISORY | Version history tables need row-count limits                       | Prune `launch_operations` to 50 most recent per profile on insert; prevents unbounded DB growth              |
| A8  | ADVISORY | DB failure must not block launch                                   | Wrap all version DB calls in `is_available()` guard; version check is informational only                     |

### Integration Challenges

1. **Non-Steam games have no version source**: The `proton_run` launch method with standalone prefixes and `native` method have no appmanifest. Version tracking for these profiles degrades to trainer-file-hash-only detection or fully manual entry. The UI must show "untracked" not "mismatch" for these (business: "untracked is not broken").

2. **Profile portability vs. version locality**: Community profiles use `portable_profile()` which strips local overrides. Version snapshots are inherently local (tied to a specific machine's Steam installation). Version data should NOT be included in portable/community exports. Community metadata fields (`game_version`/`trainer_version`) remain the sharing mechanism.

3. **Manifest path discovery**: `find_game_match()` scans steamapps to locate manifests. This requires the profile's `steam.app_id` to be populated. Profiles without `app_id` (or with stale paths) need the existing auto-populate flow to have run first (practices).

4. **Health system coupling**: Tight integration with the health pipeline means version tracking bugs could affect health scores. Extract `compute_correlation_status()` as a pure function for testability and isolate version-related health issues during initial rollout (practices).

5. **Flatpak Steam**: Some Steam Deck users run Steam via Flatpak, which may affect manifest file paths. The existing `discover_steam_root_candidates()` handles multiple root paths, but Flatpak-specific paths should be verified during testing.

### Performance Risks

1. **Manifest scanning frequency**: If health checks trigger manifest reads for all profiles, and a user has 50+ profiles, that's 50+ filesystem reads. Mitigate with caching and batching (multiple profiles may share the same manifest file — deduplicate by `app_id`).

2. **SQLite write contention**: Version snapshot updates happen at launch completion, which already writes to `launch_operations`. Group both writes in a single transaction to avoid WAL contention.

3. **Frontend re-rendering**: Adding version mismatch badges to the Profile page and Health Dashboard could increase re-render frequency. Use React memoization for version status components.

---

## Alternative Approaches

### Option A: Minimal — Health Issue Only (No New Tables)

**Description**: Don't store version snapshots at all. At launch time, read the current buildid from the manifest and compare against the buildid recorded in the last successful `launch_operations` row for this profile.

| Dimension  | Assessment                                                                                                                                                                                 |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Pros**   | Zero new tables, reuses existing launch_operations data, minimal implementation surface                                                                                                    |
| **Cons**   | Requires ALTER TABLE on launch_operations, no dedicated version state, slower queries (scanning launch_operations vs. direct lookup), can only detect changes between consecutive launches |
| **Effort** | ~2 days backend, ~1 day frontend                                                                                                                                                           |

### Option B: Hybrid — Snapshot Table + Launch History (Recommended)

**Description**: New `version_correlation` table for latest-known-good state + `game_build_id` column on `launch_operations` for history. Health system integration for alerts.

| Dimension  | Assessment                                                                                                                                                                                                                                             |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Pros**   | Fast O(1) mismatch detection via snapshot lookup, full version history via launch_operations, clean separation of "current state" vs. "history", `is_known_good` flag enables future compatibility matrix, natural extension for community integration |
| **Cons**   | New table requires migration, two write paths to maintain (snapshot + launch operation)                                                                                                                                                                |
| **Effort** | ~4 days backend, ~3 days frontend                                                                                                                                                                                                                      |

### Option C: Full Version Database — Dedicated Version Module

**Description**: New `version/` module with dedicated tables for game versions, trainer versions, version pairs, and compatibility scores. Full CRUD for version management.

| Dimension  | Assessment                                                                                                                                                                  |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**   | Most flexible for future features (version pinning, compatibility scoring, trend analysis), clean domain separation                                                         |
| **Cons**   | Over-engineered for Phase 1 needs (practices: violates KISS), more migration complexity, larger test surface, builds infrastructure for features that may never materialize |
| **Effort** | ~8 days backend, ~5 days frontend                                                                                                                                           |

### Recommendation

**Option B** is the consensus choice across all research perspectives. It provides immediate mismatch detection with minimal schema changes while preserving history for future analytics. Option A is too limited for the metadata DB's capabilities, and Option C builds infrastructure prematurely.

---

## Task Breakdown Preview

### Phase 1: Detection Foundation (~5 days)

**Task Group 1A: Manifest Enhancement** (parallelizable)

- [ ] Extend `parse_manifest()` to extract `buildid` and `LastUpdated`
- [ ] Add `build_id: Option<String>` to `SteamGameMatch`
- [ ] Update `find_game_match()` to populate build_id
- [ ] Validate buildid as numeric-only before downstream use (security A1)
- [ ] Unit tests for buildid extraction (present, missing, empty, non-numeric)

**Task Group 1B: Schema & Storage** (parallelizable with 1A)

- [ ] Migration 8→9: `version_correlation` table + `launch_operations.game_build_id` column
- [ ] `metadata/version_store.rs`: upsert/load/lookup triad (mirror `health_store.rs` pattern)
- [ ] New row type + `VerificationMethod` enum in `metadata/models.rs`
- [ ] Unit tests for snapshot CRUD operations

**Task Group 1C: Core Logic** (depends on 1A + 1B)

- [ ] `compute_correlation_status()` pure function for mismatch detection
- [ ] Integration with `record_launch_finished()` to update both `version_correlation` and `launch_operations.game_build_id`
- [ ] Version mismatch as `HealthIssue` type (warning severity)
- [ ] "Untracked" status for profiles without baseline (distinct from "mismatch")
- [ ] Unit tests for mismatch detection logic (match, mismatch, untracked, non-Steam)

**Task Group 1D: Security Fixes** (parallelizable with 1A/1B/1C)

- [ ] Add `MAX_VERSION_BYTES = 256` check in `check_a6_bounds()` for `game_version`/`trainer_version` (W1)
- [ ] Validate `pinned_commit` as hex-only, 7-64 chars in `taps.rs` (W2)
- [ ] Normalize whitespace in version comparison logic (A6)

### Phase 2: Integration (~4 days)

**Task Group 2A: Backend Integration** (depends on Phase 1)

- [ ] Startup reconciliation: batch manifest scan in `startup.rs` (async, non-blocking UI)
- [ ] Post-launch version recording in `record_launch_finished()` flow
- [ ] `commands/version.rs`: Tauri command wrappers for version queries and "Mark as Verified"
- [ ] Post-update version re-check hook in `update_game()` completion
- [ ] DB failure guard: wrap all version DB calls in `is_available()` (security A8)

**Task Group 2B: Frontend Types & Warning UI** (parallelizable with 2A)

- [ ] `src/types/version.ts`: TypeScript type definitions
- [ ] `useVersionState.ts` hook for version tracking state
- [ ] Integration with `useProfileHealth.ts` for mismatch indicators
- [ ] Layer 1: `HealthBadge` version mismatch indicator (passive badge on profile cards)
- [ ] Layer 2: Actionable banner on LaunchPage — "Launch Anyway" as primary CTA, never modal
- [ ] Layer 3: Post-launch "Did trainer work?" inline prompt (one-time per mismatch)

### Phase 3: User Experience (~5 days)

**Task Group 3A: UI Components** (depends on Phase 2)

- [ ] Version info display on ProfilesPage (show actual build IDs, not vague "incompatible")
- [ ] "Mark as Verified" action in ProfileActions (specific label, not "Dismiss")
- [ ] Trainer version hint field in ProfileFormSections
- [ ] Community data disclaimer on community-sourced version info ("community data, may not reflect your configuration" — Heroic pattern)

**Task Group 3B: Health Dashboard Enhancement** (parallelizable with 3A)

- [ ] Version mismatch column in HealthDashboardPage
- [ ] Bulk version check action ("Check All" — batch scan, deduplicate by app_id)
- [ ] Version status in HealthBadge

**Task Group 3C: Trainer mtime tracking** (parallelizable, ~1 day)

- [ ] Record trainer file `mtime` alongside SHA-256 hash in `version_correlation`
- [ ] Detect trainer-side changes independently from game buildid changes

### Phase 4: Community & Analytics (~5 days)

**Task Group 4A: Community Integration** (depends on Phase 3)

- [ ] Version compatibility data in community profile export (display-only — enforce W3 constraint)
- [ ] Community version advisory display in CommunityBrowser with disclaimer
- [ ] Version pair matching for community profile recommendations
- [ ] Extend `CommunityProfileManifest` to schema version 2 (if needed)

**Task Group 4B: Analytics** (parallelizable with 4A)

- [ ] Version history query from launch_operations
- [ ] Compatibility scoring algorithm (tiered badges aligned with ProtonDB vocabulary)
- [ ] Version timeline visualization component
- [ ] Row-count pruning: cap launch_operations to 50 most recent per profile (security A7)

### Parallelization Opportunities

- **Phase 1**: Task Groups 1A, 1B, and 1D are fully independent. 1C depends on 1A+1B completion.
- **Phase 2**: Task Groups 2A and 2B can proceed in parallel (backend integration vs. frontend warning UI).
- **Phase 3**: Task Groups 3A, 3B, and 3C are all independent work streams.
- **Phase 4**: Task Groups 4A and 4B are independent feature tracks.
- **Cross-phase**: Phase 2B can start as soon as Phase 1B completes (doesn't need 1A).

---

## Key Decisions Needed

1. **Blocking vs. non-blocking mismatch warning**: Should a version mismatch warning block the launch (modal confirmation required) or be non-blocking (banner notification)? **Recommendation: non-blocking, three-layer pattern** — mismatch doesn't guarantee incompatibility. Competitive analysis confirms: WeMod's success is built on never blocking gameplay; Vortex's blocking "incompatible" warnings are widely hated. Use passive badge → persistent banner with "Launch Anyway" CTA → post-launch verification prompt (UX, business).

2. **Trainer version source of truth**: User-entered field, community-sourced, or auto-detected from file metadata? **Recommendation: user-entered with path heuristic auto-suggest + SHA-256 file hash** for automated change detection (tech-design, practices).

3. **Version data in portable profiles**: Should version snapshots travel with profile exports? **Recommendation: No** — version data is machine-local. Community profiles already have `game_version`/`trainer_version` in their metadata for sharing (all perspectives agree).

4. **Minimum viable scope**: Phase 1+2 (detection + launch integration) vs. Phase 1+2+3 (adds UX). **Recommendation: Phase 1+2 as MVP** — resist scope inflation into community sharing (business).

5. **Health issue severity default**: Should version mismatch default to `warning` or `info`? **Recommendation: `warning`** — it's actionable and the user should notice it, but it shouldn't flag the profile as `broken`. Severity should be distinct from the `error` level used for missing paths.

6. **Table name**: `version_snapshots` vs. `version_correlation`? **Recommendation: `version_correlation`** — aligns with the feature name and avoids confusion with `health_snapshots`.

7. **Version check timing**: Pre-launch (synchronous in launch path) vs. on-demand (startup reconciliation + health dashboard)? **Recommendation: on-demand** — pre-launch manifest reads add measurable latency on Steam Deck with slow SD card storage. The codebase consistently uses on-demand patterns. Launch records version data post-completion, never gates on it (practices, security A8).

8. **Community data behavioral influence**: Should community version/compatibility data influence local mismatch warnings (suppress or trigger)? **Recommendation: absolutely not** — community data is display-only. Only local Steam manifest data + local launch history drive behavioral outcomes. This is a hard architectural constraint (security W3) that prevents malicious taps from manipulating user warnings.

---

## Open Questions

1. **How do FLiNG trainers signal their version?** The trainer executable filename sometimes includes a version suffix, but this is inconsistent. SHA-256 file hash provides a reliable change signal regardless of naming convention. Is filename heuristic parsing worth the additional complexity?

2. **What's the Steam manifest read behavior under Flatpak Steam?** Some Steam Deck users run Steam via Flatpak, which may affect manifest file paths. The existing `discover_steam_root_candidates()` handles multiple root paths, but Flatpak paths should be verified during testing.

3. **Should community taps publish version compatibility advisories?** This would require extending the `CommunityProfileManifest` schema (currently at version 1) or adding a separate advisory index format. Recommend deferring to Phase 4 scope discussion.

4. **How should non-Steam profiles handle version tracking?** The UI should show "Version tracking: not available (non-Steam profile)" for `proton_run`/`native` profiles. Trainer file hash still works for all launch methods. Consider showing manual-entry version fields for users who want to track non-Steam game versions.

5. **WeMod version history lesson — how much history to preserve?** WeMod removed version history access in May 2024 and had to revert after massive user backlash. CrossHook should ensure version history (via `launch_operations.game_build_id`) is always queryable and never silently purged. How many rows per profile is the right retention limit? (Current recommendation: 50 per profile, security A7.)

---

## Cross-Reference: Team Research Documents

| Document                | Location                | Key Contribution                                                                                                     |
| ----------------------- | ----------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Business Analysis       | `research-business.md`  | Domain complexity, "untracked is not broken" principle, scope inflation guards                                       |
| Technical Specification | `research-technical.md` | Architecture decisions (5 key decisions), schema design, TOCTOU analysis                                             |
| Security Assessment     | `research-security.md`  | W1/W2/W3 findings, A1-A8 advisories, community data trust boundary, DB failure guards                                |
| Practices Assessment    | `research-practices.md` | Reuse targets, KISS violations to avoid, module boundaries, pre-launch check rejection for Steam Deck latency        |
| UX Research             | `research-ux.md`        | Three-layer warning pattern, WeMod/Vortex/Heroic competitive analysis, "Launch Anyway" CTA, post-launch verification |
| API/External Research   | `research-external.md`  | Version detection approaches (A-D with phasing), trainer mtime, PE version extraction deferral                       |
