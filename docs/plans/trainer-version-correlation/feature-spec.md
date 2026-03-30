# Feature Spec: Trainer-Version Correlation

## Executive Summary

CrossHook will track the relationship between game versions (Steam build IDs from appmanifest ACF files) and trainer versions, detecting mismatches after game updates and warning users when trainers may be incompatible. The implementation extends the existing SQLite metadata database with a `version_snapshots` table (migration 8→9), adds `buildid` extraction to the already-functional VDF manifest parser, and surfaces mismatch data through the existing health scoring pipeline rather than creating a parallel notification subsystem. Zero new crate dependencies are required — all building blocks (`rusqlite`, `sha2`, `chrono`, VDF parser) are already in the workspace. The primary risk is trainer version inconsistency (no standardized format exists), mitigated by treating trainer versions as opaque strings supplemented with SHA-256 file hash for automated change detection.

## External Dependencies

### APIs and Services

This feature is entirely local — no web services, authentication, or network calls are required. The "APIs" are local filesystem formats:

| Interface                                | Type                                            | Status                                                                                           |
| ---------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Steam ACF manifest (`appmanifest_*.acf`) | Local filesystem — VDF/KeyValues format         | Already parsed by `steam/vdf.rs` + `steam/manifest.rs`; needs `buildid`/`LastUpdated` extraction |
| Trainer PE VERSIONINFO resource          | Local filesystem — Windows PE binary format     | Optional v2; `pelite` crate; defer                                                               |
| CrossHook SQLite metadata DB             | Local — rusqlite via existing `metadata/` layer | New `version_store.rs` module; migration 8→9                                                     |

### Libraries and SDKs

No new dependencies required. Existing crates cover all needs:

| Library              | Version        | Purpose                       | Status                                              |
| -------------------- | -------------- | ----------------------------- | --------------------------------------------------- |
| `rusqlite`           | 0.38 (bundled) | SQLite operations             | Already in `Cargo.toml`                             |
| `sha2`               | 0.10           | Trainer file hashing          | Already in `Cargo.toml` (used by `profile_sync.rs`) |
| `chrono`             | 0.4            | Timestamps                    | Already in `Cargo.toml`                             |
| `uuid`               | 1.x            | Record IDs via `db::new_id()` | Already in `Cargo.toml`                             |
| `serde`/`serde_json` | 1.x            | IPC serialization             | Already in `Cargo.toml`                             |

### External Documentation

- [Steam ACF Format Overview](https://github.com/leovp/steamfiles/blob/master/docs/acf_overview.rst): VDF key-value format, `buildid` semantics
- [Valve KeyValues format](https://developer.valvesoftware.com/wiki/KeyValues): Official VDF spec
- [notify crate docs](https://docs.rs/notify/latest/notify/): Filesystem watching (v2 consideration)

## Business Requirements

### User Stories

**Primary User: Steam Deck / Linux Gamer**

- As a Steam Deck user, I want to be warned before I launch a game+trainer combo that may no longer work after a game update, so I don't waste time troubleshooting a broken session
- As a user who auto-updates games, I want CrossHook to detect that Steam updated my game overnight and proactively flag affected profiles before I launch them
- As a Linux gamer, I want to see which game build ID was last known to work with my trainer, so I can decide whether to update the trainer or pin the game version

**Secondary User: Community Profile Author**

- As a community profile author, I want my published profile to declare the game version and trainer version it was tested against, so consumers can immediately see whether their installed game matches
- As a user with multiple trainer profiles for the same game, I want each profile tracked independently (different trainers may tolerate the same update differently)

### Business Rules

| #     | Rule                                                                                                                                     | Rationale                                                                                                        |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| BR-1  | Version snapshot recorded only on `LaunchOutcome::Succeeded`                                                                             | Don't pollute history with failed attempts                                                                       |
| BR-2  | Game version anchor is Steam manifest `buildid` (integer from `appmanifest_<appid>.acf`)                                                 | Only applies to `steam_applaunch`/`proton_run` with configured `steam.app_id`                                    |
| BR-3  | For `native` launch method or absent `steam.app_id`, version tracking is skipped silently                                                | No partial data is worse than no data                                                                            |
| BR-4  | Mismatch exists when `current_buildid != snapshot.steam_build_id` AND a snapshot exists                                                  | "No snapshot" = "untracked", not "mismatch"                                                                      |
| BR-5  | Trainer version sourced from `CommunityProfileMetadata.trainer_version` if present, otherwise trainer file SHA-256 hash as change signal | Trainer authors use inconsistent formats                                                                         |
| BR-6  | Mismatch is Warning severity, not Error — trainer may still work                                                                         | Many minor game patches don't break trainers                                                                     |
| BR-7  | Warning includes known-good build ID, current build ID, and date of last known-good launch                                               | Users need actionable context                                                                                    |
| BR-8  | Community version data is display-only — never drives behavioral outcomes (warnings, launch gates)                                       | Prevents malicious/stale community data from suppressing real warnings or creating false positives (Security W3) |
| BR-9  | Version mismatch must never block launch; "Launch Anyway" is always the primary action                                                   | WeMod lesson: users demand agency over their gameplay                                                            |
| BR-10 | "Mark as Verified" user action explicitly sets current build ID as new baseline                                                          | Clears mismatch without requiring a new launch                                                                   |
| BR-11 | Version comparison is a four-state enum: `Match`, `Mismatch`, `CommunityUnspecified`, `LocalUnknown`                                     | Only `Mismatch` triggers a warning                                                                               |
| BR-12 | Version strings from community taps bounded to 256 bytes; exceeding strings rejected at index time                                       | Prevents resource waste from oversized community data                                                            |
| BR-13 | DB failure must not block game launch — all version DB calls wrapped in availability guards                                              | Version check is informational only (Security A8)                                                                |
| BR-14 | Version snapshot data is durable and user-accessible; no automatic background pruning                                                    | WeMod v9.0 backlash when version history was silently removed                                                    |

### Edge Cases

| Scenario                                                         | Expected Behavior                                                                               | Notes                                              |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Multiple profiles for same App ID                                | Each profile has independent snapshot                                                           | Trainer A may survive an update; Trainer B may not |
| Manifest deleted or unreadable                                   | Log `Info` diagnostic, do not block launch or show warning                                      | Best-effort version tracking                       |
| Build ID rolls backward (beta branch switch)                     | Treat as mismatch — trainer was tested on a different build                                     | Don't attempt branch detection                     |
| Community profile installed without prior launch                 | `steam_build_id = NULL`, status = `untracked`, show "not yet locally verified"                  | Not a mismatch                                     |
| Profile renamed or duplicated                                    | Snapshot follows `profile_id` UUID (SQLite FK), not filename                                    | Consistent with all other metadata                 |
| User dismisses mismatch warning                                  | No data change — snapshot updated only on next successful launch or explicit "Mark as Verified" |                                                    |
| Non-Steam game (native/proton_run without app_id)                | Version tracking skipped silently; trainer hash still works                                     | `status = 'untracked'`                             |
| Trainer updated silently (same version string, different binary) | SHA-256 file hash detects the change                                                            | Hash is the automated change signal                |
| Steam update in progress (`StateFlags != 4`)                     | Skip version check, return `update_in_progress: true`, show info note                           | Avoids false mismatch during auto-update           |

### Success Criteria

- [ ] Profiles track game build ID after first successful launch (stored in SQLite, not TOML)
- [ ] Game updates detected via Steam manifest `buildid` comparison at startup and on-demand
- [ ] Users warned with non-blocking banner when profile's game version differs from installed version
- [ ] Trainer version metadata actively used for compatibility checking (not just display)
- [ ] "Launch Anyway" is always available with zero extra friction
- [ ] "Mark as Verified" allows explicit baseline reset without re-launching
- [ ] Health Dashboard reflects version mismatch as a warning-severity issue

## Technical Specifications

### Architecture Overview

```text
                               ┌──────────────────────┐
                               │   React Frontend     │
                               │  HealthDashboard     │
                               │  LaunchPanel         │
                               │  (mismatch banner)   │
                               └──────────┬───────────┘
                                          │ invoke() / listen()
                                          ▼
                               ┌──────────────────────┐
                               │  Tauri IPC Layer      │
                               │  commands/version.rs  │
                               │  commands/health.rs   │
                               │  commands/launch.rs   │
                               └──────────┬────────────┘
                                          │
                 ┌────────────────────────┼────────────────────────┐
                 │                        │                        │
                 ▼                        ▼                        ▼
      ┌──────────────────┐   ┌──────────────────────┐   ┌──────────────────┐
      │ steam/manifest.rs│   │ metadata/             │   │  launch/         │
      │ (extended)       │   │ version_store.rs (new)│   │  launch_history  │
      │ + buildid return │   │ upsert/lookup/load    │   │  (existing hook) │
      └──────────────────┘   │ compute_correlation() │   └──────────────────┘
                             └──────────┬─────────────┘
                                        ▼
                             ┌──────────────────────┐
                             │  metadata.db (SQLite) │
                             │  version_snapshots    │
                             └──────────────────────┘
```

### Data Models

#### `version_snapshots` (Migration 8→9) — Multi-Row History Table

| Field               | Type    | Constraints                                             | Description                                                                                      |
| ------------------- | ------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `id`                | INTEGER | PK AUTOINCREMENT                                        | Row identifier                                                                                   |
| `profile_id`        | TEXT    | NOT NULL, FK → `profiles(profile_id)` ON DELETE CASCADE | Links to profile; multiple rows per profile (version history)                                    |
| `steam_app_id`      | TEXT    | NULL                                                    | Steam App ID from profile. NULL for non-Steam.                                                   |
| `steam_build_id`    | TEXT    | NULL                                                    | Steam manifest `buildid` at this snapshot. NULL until first launch. Validated as numeric-only.   |
| `trainer_version`   | TEXT    | NULL, max 256 bytes                                     | User-provided or community-sourced trainer version string                                        |
| `trainer_file_hash` | TEXT    | NULL                                                    | SHA-256 hash of trainer executable. Automated change detection.                                  |
| `human_game_ver`    | TEXT    | NULL, max 256 bytes                                     | Human-readable game version from community metadata (display-only)                               |
| `status`            | TEXT    | NOT NULL, DEFAULT 'untracked'                           | `'untracked'`, `'matched'`, `'game_updated'`, `'trainer_changed'`, `'both_changed'`, `'unknown'` |
| `checked_at`        | TEXT    | NOT NULL                                                | RFC 3339 timestamp of this snapshot                                                              |

**Indexes:**

- `idx_version_snapshots_profile_checked` on (`profile_id`, `checked_at` DESC): Latest-row-per-profile queries
- `idx_version_snapshots_steam_app_id` on (`steam_app_id`): Batch lookups during manifest scan

**Key design notes:**

- Multi-row per profile — enables version timeline, trend analysis, and "last N known-good builds" queries
- Mismatch detection queries the most recent row: `WHERE profile_id = ? ORDER BY checked_at DESC LIMIT 1`
- Retention: prune to N most recent rows per `profile_id` on insert (A7 security advisory — prevents unbounded growth)
- `steam_build_id` NULL until first successful local launch
- `status = 'untracked'` is NOT a mismatch — it means no baseline exists yet

#### Pure Comparison Function

```rust
pub fn compute_correlation_status(
    stored_build_id: Option<&str>,
    current_build_id: Option<&str>,
    stored_trainer_hash: Option<&str>,
    current_trainer_hash: Option<&str>,
) -> VersionCorrelationStatus {
    // Returns: Matched | GameUpdated | TrainerChanged | BothChanged
    // NULL on either side = no change detected (can't compare)
}
```

### API Design

#### `check_version_status(name: String)` → `VersionCheckResult`

On-demand version check. Reads current manifest build ID and `StateFlags`, compares against stored snapshot. If `StateFlags != 4` (game update in progress), returns `status: "unknown"` with `update_in_progress: true` to avoid false mismatch alerts during Steam auto-updates. Fail-soft: returns `status: "unknown"` if manifest or DB unavailable.

**`VersionCheckResult` includes:**

- `has_mismatch: bool` — true when stored and current build IDs differ
- `update_in_progress: bool` — true when `StateFlags != 4` (Steam update in flight)
- `current_build_id` / `snapshot_build_id` — for delta display in warning banner

#### `set_trainer_version(name: String, trainer_version: String)` → `()`

User manually sets trainer version string. A6 bounds validated (256 bytes max).

#### `get_version_snapshot(name: String)` → `Option<VersionSnapshotInfo>`

Retrieve current version snapshot for display.

#### `acknowledge_version_change(name: String)` → `()`

"Mark as Verified" — resets status to `'matched'`, accepting current state as baseline.

**Event:** `version-scan-complete` with `{ scanned: u32, mismatches: u32 }` — emitted when startup version scan finishes.

### System Integration

#### Files to Create

- `crates/crosshook-core/src/metadata/version_store.rs`: CRUD functions + `compute_correlation_status()` pure function
- `src-tauri/src/commands/version.rs`: Tauri IPC command handlers
- `src/types/version.ts`: TypeScript types for version IPC payloads

#### Files to Modify

- `crates/crosshook-core/src/steam/manifest.rs`: Extend `parse_manifest()` to return `buildid`, `LastUpdated`, and `StateFlags` (4 = fully installed, 1026 = update in progress)
- `crates/crosshook-core/src/metadata/mod.rs`: Add `mod version_store` + MetadataStore wrapper methods
- `crates/crosshook-core/src/metadata/migrations.rs`: Add `migrate_8_to_9()` for `version_snapshots` table
- `crates/crosshook-core/src/metadata/community_index.rs`: Extend `check_a6_bounds()` for version fields (Security W1)
- `src-tauri/src/lib.rs`: Register version commands in `invoke_handler`
- `src-tauri/src/startup.rs`: Add optional version scan to reconciliation
- `src-tauri/src/commands/launch.rs`: Hook `upsert_version_snapshot()` after successful launch
- `src-tauri/src/commands/health.rs`: Extend `BatchMetadataPrefetch` and `ProfileHealthMetadata` with version fields
- `src-tauri/src/commands/community.rs`: Seed version snapshot on community import
- `src/types/health.ts`: Extend `ProfileHealthMetadata` with version fields

## UX Considerations

### User Workflows

#### Primary Workflow: Version Mismatch Detection

1. **Game updates while app is closed** — Steam auto-updates the game
2. **User opens CrossHook** — Startup scan detects `buildid` changed for affected profiles
3. **Health badges update** — Warning badge appears on affected profile cards and pinned strip
4. **User navigates to Launch page** — Actionable warning banner appears with version diff
5. **User chooses action** — "Launch Anyway" (primary), "Check Compatibility", or "Mark as Verified"

#### Error Recovery: Trainer is Broken

1. User sees mismatch warning, launches anyway
2. Trainer doesn't work with updated game
3. User returns to CrossHook, updates trainer path in Profiles page
4. Next successful launch writes new snapshot, clearing the warning

### UI Patterns

**Three-Layer Warning System** (from competitive analysis of WeMod, Vortex, Heroic):

| Layer                | Component                              | Placement                                       | Blocks Launch? |
| -------------------- | -------------------------------------- | ----------------------------------------------- | -------------- |
| 1. Indicator         | `HealthBadge` with `stale` styling (⚠) | Profile card, pinned strip, Health Dashboard    | No             |
| 2. Actionable banner | Persistent warning strip               | Top of LaunchPage                               | No             |
| 3. Soft confirmation | Inline confirmation in LaunchPanel     | Pre-launch (first time only per version change) | No             |

**Warning banner format:**

```text
┌─────────────────────────────────────────────────────────────────────┐
│ ⚠  Game updated: 1.2.3 → 1.3.0   Trainer compatibility unverified  │
│     [Launch Anyway]  [Check Compatibility]  [Mark as Verified]  [✕] │
└─────────────────────────────────────────────────────────────────────┘
```

**Language guidelines:**

- "Game was updated — trainer compatibility unverified" (not "mismatch detected")
- "Trainer may still work — launch to verify" (not "trainer is broken")
- Show specific version numbers — Vortex's vague warnings are the anti-pattern to avoid

**Five version states:**

| State                   | Badge     | Display                  | When                                             |
| ----------------------- | --------- | ------------------------ | ------------------------------------------------ |
| `version_match`         | ✓ healthy | `v1.2.3 (verified)`      | current buildid == stored buildid                |
| `version_mismatch`      | ⚠ stale   | `Game updated`           | current buildid != stored buildid                |
| `version_untracked`     | (none)    | (no badge)               | Profile never launched                           |
| `community_unspecified` | —         | `No version requirement` | Community profile has no version — NOT a warning |
| `local_unknown`         | ?         | `Version unknown`        | Cannot read Steam manifest                       |

### Accessibility Requirements

- Full gamepad/controller navigation (Steam Deck primary input)
- A button = Confirm, B button = Cancel/Dismiss, D-pad = Navigate
- Warning banner must not steal focus; soft confirmation must capture focus
- All warnings keyboard-navigable with correct tab order

### Performance UX

- **Loading States**: Version check is <10ms (local filesystem read) — no loading indicator needed
- **Non-blocking Notifications**: Persistent inline banner, not toast (toasts auto-dismiss too quickly for actionable warnings)
- **Background Startup Scan**: Run 2-3 seconds after app startup, update badges asynchronously; skip manifests with `StateFlags != 4` (update in progress) to avoid false alerts
- **Never delay launch button** while version check runs
- **Update-in-progress state**: When Steam is mid-update (`StateFlags = 1026`), show info note "Steam update in progress — version check deferred" instead of a mismatch warning

### Component Integration Points

| Component                 | Integration                                                                                                                      |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `LaunchPanel.tsx`         | Warning banner when `version_status` is `game_updated` / `trainer_changed` / `both_changed`; info note when `update_in_progress` |
| `HealthDashboardPage.tsx` | Add `version_changed` to issue categories; version status column in sortable table                                               |
| `HealthBadge.tsx`         | Reflect version mismatch in badge (reuse existing `crosshook-compatibility-badge--{rating}` CSS)                                 |
| `ProfilesPage.tsx`        | Card badge showing version status for pinned/visible profiles                                                                    |

### Frontend Hook Strategy

Version data flows through the existing health enrichment pipeline (`useProfileHealth` → `EnrichedProfileHealthReport.metadata`). A dedicated `useVersionCorrelation` hook is only warranted if version correlation gets its own standalone UI page — which UX research explicitly discourages for v1.

## Recommendations

### Implementation Approach

**Recommended Strategy**: Phased rollout with Phase 1+2 as MVP

**Phasing:**

1. **Phase 1 — Detection Foundation** (~5 days): Extend manifest parsing, SQLite migration, version_store CRUD, pure comparison function, security fixes (W1, W2)
2. **Phase 2 — Launch Integration** (~4 days): Tauri commands, startup reconciliation, health enrichment, frontend warning banner, TypeScript types
3. **Phase 3 — User Experience** (~4 days): Version display on Launch/Profile pages, "Mark as Verified", trainer version hint field, Health Dashboard enhancement
4. **Phase 4 — Community & Analytics** (~5 days): Community version enrichment, version history from launch_operations, compatibility scoring

### Technology Decisions

| Decision                 | Recommendation                                   | Rationale                                                           |
| ------------------------ | ------------------------------------------------ | ------------------------------------------------------------------- |
| Version storage          | SQLite multi-row history table (no TOML changes) | Enables trend analysis and version timeline; profiles stay portable |
| Game version source      | Steam `buildid` (integer equality)               | No semver needed — buildid is opaque monotonic integer              |
| Trainer change detection | SHA-256 file hash                                | `sha2` already in Cargo.toml; catches silent updates                |
| Mismatch surfacing       | Health pipeline integration                      | Reuses existing BatchMetadataPrefetch + HealthBadge                 |
| Capture timing           | Post-launch success + startup reconciliation     | Avoids launch path latency                                          |
| Comparison semantics     | Pure function `!=` equality                      | No semver parser, no version ranges — KISS                          |

### Quick Wins

- **Buildid extraction** (~10 lines of Rust): New `parse_manifest_full()` with `get_child("buildid")` — unlocks entire feature
- **A6 bounds fix** (Security W1, ~4 lines): Add `MAX_VERSION_BYTES = 256` check for version fields in `check_a6_bounds()`
- **Community metadata display**: Surface existing `game_version`/`trainer_version` more prominently in `CommunityBrowser.tsx`

### Future Enhancements

- **Proton version correlation**: Track Proton version alongside game/trainer versions
- **Compatibility scoring**: Compute confidence scores based on launch success rates per version pair
- **Version pinning**: Allow users to pin known-good version pairs
- **SteamDB/PCGamingWiki integration**: Cache version-to-changelog mappings via `external_cache_entries` table
- **Community version advisory feed**: Community taps publish version compatibility advisories

## Risk Assessment

### Technical Risks

| Risk                                               | Likelihood | Impact | Mitigation                                                             |
| -------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------- |
| Steam manifest format changes                      | Low        | High   | VDF parser is schema-agnostic; missing buildid returns None gracefully |
| Trainer version inconsistency (no standard format) | High       | Medium | Treat as opaque strings + SHA-256 hash for automated change detection  |
| False positive mismatch alerts                     | Medium     | Medium | Default to `warning` severity; "Untracked" distinct from "Mismatch"    |
| Steam beta branch buildid ambiguity                | Medium     | Low    | Show actual build IDs; don't attempt branch detection                  |
| Database migration failure                         | Low        | High   | Idempotent `IF NOT EXISTS`; follow existing migration pattern          |
| Manifest read errors during launch                 | Medium     | Low    | Non-blocking, fail-soft; Steam doesn't exclusively lock ACF on Linux   |

### Integration Challenges

- **Non-Steam games**: No version source — degrade gracefully to trainer-hash-only or skip
- **Profile portability**: Version data is machine-local, must NOT be included in portable exports
- **Manifest path discovery**: Requires `steam.app_id` populated; existing `find_game_match()` handles scanning
- **Health system coupling**: Extract `compute_correlation_status()` as pure function for testability and isolation

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| Finding                                                               | Risk                                                              | Mitigation                                                    | Alternatives |
| --------------------------------------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------- | ------------ |
| W1: `game_version`/`trainer_version` unbounded in `check_a6_bounds()` | Resource waste from oversized community strings                   | Add `MAX_VERSION_BYTES = 256` check (~4 lines)                | —            |
| W2: `pinned_commit` not validated before git subprocess               | Git flag injection via `-flag`-shaped commit hash                 | Validate hex-only, 7-64 chars before passing to git           | —            |
| W3: Community version data must not control behavioral outcomes       | Malicious/stale tap data could suppress or trigger false warnings | Hard architectural constraint: community data is display-only | —            |

#### Advisories — Best Practices

- A1: Validate `buildid` as numeric-only before storage (deferral: low risk since VDF parser is trusted)
- A5: Version comparison must not panic on malformed input — use `Result`-returning parse
- A6: Normalize whitespace (`.trim()`) on both sides before comparing
- A7: Version history table requires row-count retention limits — prune to N most recent per `profile_id` on each insert (required, not deferred, since multi-row history was chosen)
- A8: DB failure must not block launch — wrap all version calls in `is_available()` guard

## Task Breakdown Preview

### Phase 1: Detection Foundation

**Focus**: Core Rust infrastructure for version tracking
**Tasks**:

- Add `parse_manifest_full()` alongside existing `parse_manifest()` — returns `ManifestData` with `build_id`, `last_updated`, `state_flags`
- Migration 8→9: `version_snapshots` multi-row history table with retention pruning
- `metadata/version_store.rs`: insert/load/lookup-latest triad + per-profile retention pruning
- `compute_correlation_status()` pure function
- Security fixes: A6 bounds (W1), pinned_commit validation (W2), buildid numeric validation (A1)
- Unit tests for all new functions
  **Parallelization**: Task Groups 1A (manifest), 1B (schema/storage), and 1D (security) are fully independent

### Phase 2: Launch Integration

**Focus**: Wire version tracking into Tauri commands and frontend
**Dependencies**: Phase 1 complete
**Tasks**:

- Tauri IPC commands: `check_version_status`, `set_trainer_version`, `get_version_snapshot`, `acknowledge_version_change`
- Post-launch success hook in `commands/launch.rs`
- Startup reconciliation scan in `startup.rs`
- Health enrichment: extend `BatchMetadataPrefetch` and `ProfileHealthMetadata`
- Frontend: TypeScript types, warning banner component, health badge integration
  **Parallelization**: Backend commands and frontend types can proceed in parallel

### Phase 3: User Experience

**Focus**: Polish version display and user interaction
**Tasks**:

- Version info display on Launch page and Profile page
- "Mark as Verified" action in ProfileActions
- Trainer version hint field in ProfileFormSections
- Health Dashboard: version mismatch column, bulk check action
- Community import: seed version snapshot from community metadata

### Phase 4: Community & Analytics

**Focus**: Community sharing and version intelligence
**Tasks**:

- Community version data enrichment and display
- Version history queries from `launch_operations.game_build_id`
- Compatibility scoring based on version pair success rates
- Community tap version advisory integration (schema v2)

## Decisions (Resolved)

1. **`parse_manifest()` return type migration** — **Decision: Add `parse_manifest_full()` alongside**
   - Keep existing `parse_manifest()` untouched for current callers
   - Add `parse_manifest_full()` returning `ManifestData` with `build_id`, `last_updated`, `state_flags`
   - Migrate callers incrementally in later phases if desired

2. **Non-Steam profile behavior** — **Decision: Skip for now**
   - Non-Steam profiles (`native`/`proton_run` without `steam.app_id`) get `status = 'untracked'`
   - No trainer-hash-only tracking in v1; revisit if demand exists

3. **Version history depth** — **Decision: Multi-row history table**
   - Use a multi-row `version_snapshots` table with per-profile retention limits (A7: prune to N most recent per `profile_id` on insert)
   - Enables trend analysis, version timeline, and "last N known-good builds" queries
   - Schema uses `(profile_id, checked_at)` composite key or auto-increment PK with `profile_id` FK
   - Mismatch detection queries the most recent row per profile (`ORDER BY checked_at DESC LIMIT 1`)
   - Note: This departs from the single-row `health_snapshots` pattern; the research team recommended single-row but user chose multi-row for richer version intelligence

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Steam manifest format, trainer version sources, Rust crates, filesystem monitoring
- [research-business.md](./research-business.md): User stories, business rules, domain model, workflows, codebase integration
- [research-technical.md](./research-technical.md): Architecture, SQLite schemas, Tauri commands, system integration details
- [research-ux.md](./research-ux.md): Warning patterns, competitive analysis (WeMod, Vortex, Heroic, ProtonDB), gamepad navigation
- [research-security.md](./research-security.md): Severity-leveled security findings (0 CRITICAL, 3 WARNING, 8 ADVISORY)
- [research-practices.md](./research-practices.md): Code reuse targets, KISS assessment, modularity design, build-vs-depend analysis
- [research-recommendations.md](./research-recommendations.md): Phasing strategy, risk assessment, alternative approaches, task breakdown
