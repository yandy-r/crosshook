# Business Analysis: Trainer-Version Correlation

## Executive Summary

CrossHook users need to know when a game update may have broken their trainer. Steam stores a monotonically increasing integer `buildid` in each game's `appmanifest_*.acf` file — this is the ground truth for "game was updated." The metadata SQLite database already supports the pattern needed: record a version snapshot on successful launch, then compare the current build ID against that snapshot at launch time and at startup. This is strictly additive infrastructure; no TOML profile mutations are required.

---

## User Stories

### Steam Deck / Casual Users

- **As a Steam Deck user**, I want to be warned before I launch a game+trainer combination that may no longer work after a game update, so I don't waste time troubleshooting a broken session.
- **As a user who auto-updates games**, I want CrossHook to detect that Steam updated my game overnight and proactively flag the relevant profiles before I launch them.

### Linux Power Users

- **As a Linux gamer**, I want to see which game build ID was last known to work with my trainer, so I can decide whether to pin the game version or wait for a trainer update.
- **As a user with multiple trainer profiles for the same game**, I want each profile tracked independently (different trainers may tolerate the same update differently).

### Community Profile Authors

- **As a community profile author**, I want my published profile to declare the game version and trainer version it was tested against, so consumers can immediately see whether their installed game matches.
- **As a contributor**, I want the system to record when a community profile's version combo was last validated by someone in the community.

---

## Business Rules

### Core Rules

| #    | Rule                                                                                                                                                                   | Rationale                                                                                             |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| BR-1 | A **version snapshot** is recorded only on `LaunchOutcome::Succeeded`                                                                                                  | Recording on launch-start would pollute history with failed attempts                                  |
| BR-2 | The game version anchor is the Steam manifest `buildid` integer, read from `appmanifest_<appid>.acf`                                                                   | Only applies to `steam_applaunch` and `proton_run` methods with a configured `steam.app_id`           |
| BR-3 | For `native` launch method or absent `steam.app_id`, version tracking is skipped silently                                                                              | No partial data is worse than no data                                                                 |
| BR-4 | A mismatch exists when `current_buildid != snapshot.steam_build_id` AND a snapshot exists                                                                              | "No snapshot" is not a mismatch — it's "untracked"                                                    |
| BR-5 | `trainer_version` in a version snapshot is sourced from `CommunityProfileMetadata.trainer_version` if present, otherwise from trainer file `mtime` as a fallback proxy | Trainer authors use inconsistent filenames; mtime is a reliable change signal                         |
| BR-6 | Mismatch is a **Warning** severity, not an Error — the trainer may still work                                                                                          | Many minor game patches don't break trainers                                                          |
| BR-7 | A mismatch warning must include the known-good build ID, current build ID, and date of last known-good launch                                                          | Users need actionable context, not just "something changed"                                           |
| BR-8 | Version snapshot history is retained (not overwritten) — the latest successful snapshot is used for comparison                                                         | Supports "this worked on build 12345, then broke at 12678, then worked again at 12701" trend analysis |

### Edge Cases

- **Multiple profiles for the same App ID**: Each profile has its own snapshot. Trainer A may survive an update; Trainer B may not.
- **Game installed in multiple Steam libraries**: Manifest is found via `find_game_match()` which already handles deduplication. If ambiguous, skip version tracking for that profile.
- **Manifest deleted or unreadable**: Emit `Info` diagnostic log entry, do not block launch or show warning.
- **Build ID rolls backward** (e.g., Steam beta branch switched off): Treat as a mismatch — the trainer was tested on a higher build.
- **Community profile installed without prior launch**: Snapshot contains `game_version` (string) from `CommunityProfileMetadata` as a human reference, but no `steam_build_id` until first successful launch on this machine.
- **Profile renamed or duplicated**: The snapshot follows the `profile_id` UUID (via SQLite FK), not the filename, consistent with all other metadata patterns.
- **User dismisses mismatch warning**: No business logic change — the snapshot is only updated on next successful launch.

---

## Workflows

### Primary Workflow: Version Snapshot on Successful Launch

```
[Launch completes with Succeeded outcome]
  └─> Does profile have steam.app_id?
        No  ──> skip silently
        Yes ──> Find manifest path for app_id (reuse steam/manifest.rs logic)
              └─> Parse manifest buildid field
                    ├─ Unreadable ──> log Info diagnostic, skip
                    └─ Readable ──> upsert version_snapshot(
                                      profile_id,
                                      steam_build_id,
                                      trainer_version (from metadata or mtime),
                                      launched_at
                                    )
```

### Primary Workflow: Mismatch Check Before Launch

```
[User initiates launch]
  └─> Does profile have steam.app_id AND a version snapshot?
        No  ──> launch normally (no history to compare)
        Yes ──> Read current manifest buildid
              └─> current_buildid == snapshot.steam_build_id?
                    Yes ──> launch normally
                    No  ──> emit MismatchWarning {
                              profile_name,
                              last_good_build_id,
                              current_build_id,
                              last_good_date,
                              trainer_version
                            }
                          └─> UI presents non-blocking warning with "Launch anyway" CTA
```

### Secondary Workflow: Passive Startup Scan

```
[App startup, after metadata reconciliation]
  └─> For each profile with steam.app_id AND a version snapshot:
        Read manifest buildid (non-blocking, parallel or deferred)
        If mismatch: mark profile in metadata as needs_version_check = true
  └─> UI reflects staleness badge on affected profiles (same as health badges)
```

### Workflow: Community Profile Installation

```
[User installs community profile]
  └─> CommunityProfileMetadata.game_version → stored as human_reference in snapshot
  └─> CommunityProfileMetadata.trainer_version → stored as trainer_version in snapshot
  └─> steam_build_id = NULL (not known until first successful local launch)
  └─> UI shows "version not yet locally verified" indicator
```

### Error Recovery

- Manifest unreadable at launch → log only, do not block, do not update snapshot
- Database write fails during snapshot → log warning, do not block launch
- Snapshot lookup fails → treat as "untracked", proceed without mismatch check

---

## Domain Model

### Key Entities

**`VersionSnapshot`** — the core new entity

```
profile_id       TEXT FK → profiles(profile_id)
steam_build_id   INTEGER | NULL   -- NULL until first successful local launch
trainer_version  TEXT | NULL      -- from CommunityProfileMetadata or heuristic
human_game_ver   TEXT | NULL      -- from CommunityProfileMetadata.game_version (display only)
launched_at      TEXT             -- RFC3339, last successful launch with this combo
created_at       TEXT
```

**`GameBuildId`** — a value object (not a table)

- Source: `buildid` field in `appmanifest_<appid>.acf` (integer)
- Read by: extending `parse_manifest()` in `steam/manifest.rs` to return `buildid` alongside `appid`/`installdir`

**`TrainerVersion`** — a value object (not a table)

- Primary source: `CommunityProfileMetadata.trainer_version` (free-text string)
- Fallback: file `mtime` as Unix timestamp (change signal only, not human-readable)
- Comparison: string equality for text version; numeric inequality for mtime

**`VersionMismatchWarning`** — an ephemeral struct (not persisted)

- `profile_name`, `last_good_build_id`, `current_build_id`, `last_good_date`, `trainer_version`

### State Transitions for `VersionSnapshot`

```
[Profile Created / Community Profile Installed]
  └─> version_snapshot row: steam_build_id=NULL, trainer_version=from_community_or_null
      State: "untracked"

[First Successful Launch]
  └─> version_snapshot row: steam_build_id=<current>, trainer_version=<current>, launched_at=now
      State: "tracked"

[Game Updated (buildid changes)]
  └─> No snapshot change (snapshot still holds last-good buildid)
      State: "mismatch_pending" (detected at next launch attempt)

[Successful Launch After Update]
  └─> version_snapshot row: steam_build_id=<new_current>, launched_at=now
      State: "tracked" (resolved)
```

### Relationship to Existing Entities

```
profiles (1) ──── (0..1) version_snapshots   [new, mirrors health_snapshots pattern]
profiles (1) ──── (0..N) launch_operations   [existing, used to determine Succeeded]
community_profiles (0..1) ──── (0..1) profiles  [community install → profile creation]
```

---

## Existing Codebase Integration

### Steam Manifest (`steam/manifest.rs`)

- `parse_manifest()` returns `(appid, installdir)` — needs to also return `buildid`
- `buildid` is a sibling VDF key under `AppState`, same parse logic
- `VdfNode::get_child("buildid")` would work with existing parser
- `find_game_match()` returns `SteamGameMatch` with `manifest_path` — callers already have the path needed

### Metadata Database (`metadata/`)

- Current schema version: 8. Version tracking needs migration 9 to add `version_snapshots` table.
- `health_snapshots` is the direct pattern to follow: `INSERT OR REPLACE` on profile_id PK
- `launch_history.rs::record_launch_finished()` is the hook point — after `LaunchOutcome::Succeeded`, call `upsert_version_snapshot()`
- `profile_sync.rs::lookup_profile_id()` is already used in launch history — same pattern needed for version store
- **Critical constraint**: `steam.app_id` is stored in the TOML profile but is NOT a column in the `profiles` metadata table (only `game_name` and `launch_method` are promoted). To resolve the manifest path at snapshot time, the Tauri command layer must pass the resolved `app_id` explicitly, or the version store must accept it as a parameter rather than joining from metadata.

### Profile System (`profile/`)

- No TOML model changes required — version data lives entirely in SQLite
- `GameProfile` does NOT get new fields
- `CommunityProfileMetadata.game_version` / `trainer_version` are already the community-contributed display strings

### Health System (`profile/health.rs`)

- Version mismatch is a **separate concern from path health** — should not be merged into `check_profile_health()`
- Rationale: health checks are synchronous path/file validations; version checks require async manifest reads and database I/O
- Integration point: `ProfileHealthReport` may be extended with an optional `version_status` field, OR version warnings are surfaced separately in the UI

### Tauri Commands (`src-tauri/src/commands/`)

- New Tauri commands needed: `get_version_status(profile_name)`, `dismiss_version_warning(profile_name)`
- Existing launch commands in `launch.rs` will need to call `upsert_version_snapshot` post-success
- `health.rs` Tauri commands provide the structural template

### Community Index (`metadata/community_index.rs`)

- `game_version` and `trainer_version` already stored in `community_profiles` table
- When installing a community profile, these values flow into the initial `version_snapshot`

---

## Success Criteria

Mapped to GitHub issue #41 acceptance criteria:

| Criterion                                  | Implementation Path                                                                          |
| ------------------------------------------ | -------------------------------------------------------------------------------------------- |
| Profiles track game build ID at save time  | Record on first successful launch, not at TOML save (TOML-free approach)                     |
| Game updates detected via manifest changes | `buildid` read from ACF at launch time; compared against stored snapshot                     |
| Users warned on version mismatch           | Pre-launch check emits `MismatchWarning`; UI shows non-blocking warning with "Launch anyway" |
| Trainer version metadata actively used     | `trainer_version` from `CommunityProfileMetadata` included in snapshot; shown in warning     |

---

## Teammate Input: Clarifications and Constraints

The following constraints were surfaced by the tech-designer and recommendations-agent during review:

### Constraints Confirmed

- `steam.app_id` is NOT promoted to the metadata `profiles` table — version store must receive it from the Tauri command layer at snapshot time (see Metadata Database section above).
- `MetadataStore` uses `Arc<Mutex<Connection>>` — all DB operations are serialized; the new `upsert_version_snapshot()` must be efficient (single INSERT OR REPLACE, no scans).
- Trainer version auto-detection is **not possible** for FLiNG/WeMod executables programmatically. Trainer version is strictly user-asserted or community-contributed. The `mtime` fallback is a change-signal heuristic only, not a version identifier.

### Business Rule Refinements

**BR-1 revised**: A version snapshot records that a launch **started successfully** (process spawned without crash). It does NOT verify that trainer functions activated correctly. Language in UI must reflect "last known compatible launch" not "verified working."

- Reason: `LaunchOutcome::Succeeded` = clean process exit, not trainer functionality validation. Explicit user confirmation ("trainer worked") would require a post-session prompt — this is scope expansion for Phase 2.

**New BR-9**: The game update workflow (`update/service.rs`) has no post-update hook. After a game update completes, the existing version snapshot for that profile becomes stale. The update flow should trigger a version re-scan / snapshot invalidation. Specifically: when `update_game()` succeeds, clear or mark the `version_snapshot.steam_build_id` as needing re-verification.

**New BR-10**: User-local version tracking data (which build IDs launched successfully) must NOT automatically flow back to community taps. Community tap updates require explicit user action (PR to tap repo). Local snapshots are local-only.

**New BR-11 (from security-researcher)**: Version comparison result is a **four-state enum**, not a binary match/mismatch:

- `Match` — `current_buildid == snapshot.steam_build_id`
- `Mismatch` — `current_buildid != snapshot.steam_build_id` AND snapshot exists with a non-null build ID
- `CommunityUnspecified` — community profile's `game_version`/`trainer_version` is empty; no comparison is possible or expected
- `LocalUnknown` — no local snapshot exists yet (profile never successfully launched on this machine)

Only `Mismatch` triggers a user-visible warning. `CommunityUnspecified` is not an error condition.

**New BR-12 (from security-researcher)**: `game_version` and `trainer_version` strings from community tap manifests are bounded to **256 bytes**. Strings exceeding this limit are rejected at tap indexing time — the profile entry is skipped with a warning log entry (same handling as other validation violations during indexing). This is enforced in `metadata/community_index.rs` at write time, not at read/comparison time.

**New BR-13 (from security-researcher)**: Community-sourced version strings are **informational and display-only**. They must never be used to construct file paths, subprocess arguments, environment variables, or shell script content. Business logic may only use them for string comparison and UI display.

**New BR-14 (from api-researcher)**: Trainer version auto-extraction from `.exe` binaries is not a required capability for Phase 1. FLiNG/WeMod trainers have no sidecar metadata file; PE VERSIONINFO resource parsing is unreliable and adds significant complexity. The trainer version in a `version_snapshot` must come from one of two explicit sources: (a) `CommunityProfileMetadata.trainer_version` at community profile install time, or (b) a future user-confirmation prompt (Phase 2). Trainer file `mtime` is recorded as a change-detection signal only — it is never surfaced as a "trainer version" to the user.

**New BR-15 (from api-researcher)**: Phase 2 community schema extension — `CommunityProfileMetadata` should gain a `game_buildid` integer field (the exact Steam build ID the community author tested against). This is more precise than the existing free-text `game_version` string for machine comparison. Absence of `game_buildid` in a manifest = `CommunityUnspecified` (BR-11). This requires a community profile schema version bump.

**New BR-16 (from ux-researcher)**: Version mismatch must never block launch. "Launch anyway" is always available with zero extra friction. This is a hard UX constraint, not just a preference.

**New BR-17 (from ux-researcher)**: A "Mark as Verified" action must exist — allowing the user to explicitly set the current game build ID as the new baseline without requiring a new launch. This maps to an explicit `upsert_version_snapshot()` call with the current manifest `buildid`, triggered by user intent. It immediately clears `Mismatch` state.

**New BR-18 (from ux-researcher)**: A post-launch "Did the trainer work?" confirmation prompt (optional, non-blocking) updates the baseline on "Yes". On "No", it escalates the version status toward a `broken`-adjacent state. This is **Phase 2** — requires a new status value and user-prompt flow not present in Phase 1.

**New BR-19 (from ux-researcher)**: `launch_anyway_count` — the number of times a user launched despite an active `Mismatch` warning — must be tracked in SQLite (as a column in `version_snapshots`), NOT in the TOML profile. This enables future escalation logic ("after N ignored warnings, surface a stronger prompt").

**New BR-20 (from ux-researcher — WeMod lesson)**: Version snapshot data must be durable and user-accessible. Deleting or pruning version records requires explicit user action. Automatic background pruning of version history is prohibited. Reference: WeMod v9.0 silently removed version history access → significant user backlash → restored in v9.0.2.

**CONFLICT RESOLVED — ux-researcher `last_verified_game_version` vs. no-TOML constraint**: The ux-researcher proposed a `last_verified_game_version` field in the TOML `GameProfile` struct. This conflicts with the established constraint (tech-designer + metadata architecture) that version data lives **entirely in SQLite, not in TOML**. Resolution: `last_verified_game_version` is stored as `steam_build_id` in the `version_snapshots` SQLite table. No new TOML fields are added. The Tauri command layer reads from SQLite and supplies the value to the UI on demand.

---

## Open Questions

1. **Should the version snapshot table keep full history or be a single-row-per-profile upsert?**
   - `health_snapshots` uses single-row (INSERT OR REPLACE) — simple and sufficient for "last known good"
   - Full history would enable trend analysis but is scope expansion
   - **Recommendation**: start with single-row (migration is simple); history can be added in a follow-up

2. **How to handle the case where the trainer file is updated but `trainer_version` string is unchanged?**
   - Store trainer file `mtime` as `trainer_mtime` alongside `trainer_version`
   - Mtime change + same version string = log info but don't warn (version author may not have bumped version)

3. **Should mismatch detection be blocking or dismissible?** _(resolved)_
   - **Decision**: non-blocking, always dismissible per BR-16. "Launch anyway" is always present.
   - No permanent per-build dismiss in Phase 1 — each app session re-checks. `launch_anyway_count` (BR-19) tracks repeated ignoring for future escalation.

4. **Does the passive startup scan add perceptible latency?**
   - Reading N manifests at startup could be slow if a user has many profiles
   - **Recommendation**: run as background task via Tauri `async_runtime::spawn`, update UI via event

5. **What is the community sharing mechanism for version records?**
   - Issue #41 mentions "community integration for sharing version compatibility data"
   - Current community taps are git repos with profile manifests — extending `CommunityProfileMetadata` with a `game_buildid` integer field (the actual Steam build ID the author tested against) is more precise than the existing free-text `game_version` string
   - This is a meaningful but bounded scope addition: one new field in the community schema, one new column in `community_profiles` SQLite table
   - **Recommendation**: treat as Phase 2 (community authors must opt in by adding `game_buildid` to their manifests; absence of the field = `CommunityUnspecified` state per BR-11)

6. **How to pass `steam.app_id` to version store at snapshot time?**
   - Option A: Tauri launch command already has the resolved profile; pass `steam.app_id` explicitly to `upsert_version_snapshot()` as a parameter
   - Option B: Add `steam_app_id` column to the `profiles` metadata table (migration 9a) and promote it during `sync_profiles_from_store()`
   - **Recommendation**: Option A (simpler, no metadata schema change to `profiles` table; `steam_app_id` is volatile and machine-specific anyway)
