# CrossHook Feature Implementation Guide

**Tracking issue**: #78
**Research source**: docs/research/additional-features/deep-research-report.md
**Last updated**: 2026-03-31

This document provides a recommended implementation order, dependency map, and quick-win guide for the features identified in the deep research analysis. Use it alongside the GitHub issues to plan sprints.

---

## Guiding Principle

> Invest in depth over breadth. Every feature should make trainer orchestration more reliable, diagnosable, or shareable -- not expand CrossHook into a general-purpose launcher.

### Storage Design Checkpoint (Mandatory)

For every issue scoped from this guide, explicitly document storage ownership before implementation:

- Classify each new/changed datum as one of:
  - user-editable preferences (`settings.toml`)
  - operational/history/cache metadata (SQLite metadata DB)
  - ephemeral runtime-only state (in-memory)
- Add a short "persistence and usability" note in the issue/plan covering:
  - migration/backward compatibility expectations
  - offline behavior expectations
  - degraded/failure fallback behavior
  - what users can view and edit directly

#### SQLite Metadata DB — current state (schema v13, live)

The SQLite metadata DB is **live in production** at `~/.local/share/crosshook/metadata.db` (WAL mode, `0600` permissions). Access via `MetadataStore::try_new()` in `crosshook-core`. New schema migrations go in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`.

**Existing tables and their relevance to upcoming Phase 6 / P2 features:**

| Table                                                             | Since | Relevant to                                                                                                                                              |
| ----------------------------------------------------------------- | ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `profiles`                                                        | v1    | All features; source of truth for profile identity                                                                                                       |
| `launch_operations`                                               | v3    | #36 (diagnostics), #49 (bundle export) — already captures launch history and `diagnostic_json`                                                           |
| `community_taps` / `community_profiles`                           | v4    | #59 (tap pinning) — `pinned_commit` field added to tap subscription                                                                                      |
| `external_cache_entries`                                          | v4    | **#53 (ProtonDB lookup)** — reuse this table for ProtonDB API responses (cache key `protondb:report:v1:{app_id}`); mirrors the pattern documented in #52 |
| `health_snapshots`                                                | v6    | #38 (health dashboard), #61 (prefix health)                                                                                                              |
| `version_snapshots`                                               | v9    | #41 (version correlation), #63 (hash verification) — `trainer_file_hash` column already exists                                                           |
| `bundled_optimization_presets` / `profile_launch_preset_metadata` | v10   | #50 (optimization presets) — infrastructure already present                                                                                              |
| `config_revisions`                                                | v11   | #46 (config history/rollback) — TOML snapshots with SHA-256 already being stored                                                                         |
| `optimization_catalog`                                            | v12   | #66 (data-driven catalog) — table exists; populate/extend here                                                                                           |
| `trainer_hash_cache`                                              | v13   | #63 (trainer hash verification) — SHA-256 per trainer per profile already tracked                                                                        |
| `offline_readiness_snapshots` / `community_tap_offline_state`     | v13   | #44 (offline-first) — readiness state already captured                                                                                                   |

**Features that can leverage existing tables without a new migration:**

- #53 ProtonDB lookup → `external_cache_entries` (same generic HTTP cache pattern as #52)
- #63 Trainer hash verification → `trainer_hash_cache` (already at v13)
- #46 Config history → `config_revisions` (already at v11)
- #44 Offline-first → `offline_readiness_snapshots` + `community_tap_offline_state` (already at v13)

**Features that will require a new migration (next schema bump):**

- #52 Game metadata / cover art → new `game_image_cache` table (filesystem blobs + DB metadata row); `external_cache_entries` for metadata JSON only
- #61 Prefix health monitoring → likely new columns or table for prefix size snapshots and scan timestamps
- Any other feature that persists net-new structured data not covered by an existing table

---

## Quick Wins

These features require minimal effort because the infrastructure already exists. They can be completed in a day or less each and provide immediate user-facing value.

| #   | Feature                       | Status | Effort | What To Do                                                                                | Key Files                                                                                                                                          |
| --- | ----------------------------- | ------ | ------ | ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| #39 | Actionable validation errors  | Done   | Hours  | Shipped structured launch validation help/severity metadata and LaunchPanel guidance UI   | `crates/crosshook-core/src/launch/request.rs`, `src-tauri/src/commands/launch.rs`, `src/hooks/useLaunchState.ts`, `src/components/LaunchPanel.tsx` |
| #56 | Profile duplicate / clone     | Done   | Hours  | Add Tauri command: `load(name)` + `save(new_name)` with conflict check                    | `src-tauri/src/commands/profile.rs`, `ProfileActions.tsx`                                                                                          |
| #55 | Community profile export      | Done   | Hours  | Add Tauri command wrapping existing `export_community_profile()` + UI button              | `src-tauri/src/commands/community.rs`                                                                                                              |
| #64 | Stale launcher detection      | Done   | Hours  | Implement real `is_stale` logic: compare launcher paths vs current profile                | `crates/crosshook-core/src/export/launcher_store.rs`                                                                                               |
| #54 | Adaptive Deck Mode layout     | Done   | Hours  | CSS custom properties keyed on `data-crosshook-controller-mode` attribute                 | `src/styles/variables.css`, `src/styles/theme.css`                                                                                                 |
| #59 | Tap pinning                   | Done   | Hours  | Add `pinned_commit: Option<String>` to `CommunityTapSubscription`, gate `fetch_and_reset` | `crates/crosshook-core/src/community/taps.rs`                                                                                                      |
| #58 | Extended optimization catalog | Done   | Hours  | Add 8 new entries to `LAUNCH_OPTIMIZATION_DEFINITIONS` array                              | `crates/crosshook-core/src/launch/optimizations.rs`                                                                                                |

**Recommended approach**: Batch these into a single sprint or PR series. Each is independently shippable.

---

## Recommended Implementation Order

Features are ordered by dependency chains, progressive value delivery, and effort sequencing. Complete each phase before moving to the next.

### Phase 1: Foundation (Error Communication)

_Goal: Users understand what happened when something fails._

```
 #39 Actionable validation errors   ──┐
                                      ├──> Phase 1 complete
 #40 Dry run / preview launch       ──┘
```

| Order | Issue                                   | Status | Rationale                                                                                                                                                              |
| ----- | --------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1     | **#39** -- Actionable validation errors | Done   | Lowest effort P0. Immediate UX improvement. Sets the pattern for all error communication.                                                                              |
| 2     | **#40** -- Dry run / preview launch     | Done   | All computation functions are pure and side-effect-free. Wire `validate()` + `resolve_launch_directives()` + `build_steam_launch_options_command()` into a preview UI. |

**Dependencies**: None. These are standalone improvements.
**Estimated effort**: 2-3 days total.

---

### Phase 2: Diagnostics & Health (Reliability Layer)

_Goal: Users know when things are broken and why._

```
 #39 (Phase 1) ──> #36 Post-launch diagnostics ──┐
                                                   ├──> #49 Diagnostic bundle
 #38 Profile health dashboard ────────────────────┘
```

| Order | Issue                                      | Status | Rationale                                                                                                                              |
| ----- | ------------------------------------------ | ------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| 3     | **#36** -- Post-launch failure diagnostics | Done   | Builds on #39's error communication pattern. Adds exit code analysis, Proton error detection, crash report collection.                 |
| 4     | **#38** -- Profile health dashboard        | Done   | Batch `validate()` across all profiles. Reuses the validation help text from #39. Surface health in sidebar.                           |
| 5     | **#49** -- Diagnostic bundle export        | Done   | Combines outputs from #36 (launch logs) and #38 (profile health) into a shareable archive. Natural capstone for the diagnostics phase. |

**Dependencies**: #39 should be done first (establishes error patterns). #36 and #38 can run in parallel.
**Estimated effort**: 1-2 weeks total.

---

### Phase 3: Profile Infrastructure (Core Improvements)

_Goal: Profiles are robust, portable, and easy to manage._

```
 Quick wins: #56 clone, #55 export, #64 stale launchers
                                                          ──> #42 Override layers ──> #45 Import wizard
 #47 Pinned profiles / favorites
 #48 Proton version migration
```

| Order | Issue                                  | Status | Rationale                                                                                                                                 |
| ----- | -------------------------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| 6     | **#56** -- Profile clone               | Done   | Quick win. Unblocks #50 (optimization presets need easy profile variants).                                                                |
| 7     | **#55** -- Community export from GUI   | Done   | Quick win. Backend function exists.                                                                                                       |
| 8     | **#64** -- Stale launcher detection    | Done   | Quick win. Flip `is_stale` from always-false to real comparison.                                                                          |
| 9     | **#47** -- Pinned profiles / favorites | Done   | Small `AppSettingsData` extension. High UX value on Steam Deck.                                                                           |
| 10    | **#48** -- Proton migration tool       | Done   | Detect stale Proton paths, suggest replacements from discovery. Natural extension of #38 health dashboard.                                |
| 11    | **#42** -- Profile override layers     | Done   | The biggest single improvement for community profile adoption. Split portable base from local paths.                                      |
| 12    | **#45** -- Import wizard               | Done   | Depends on #42 (override layers) to properly separate portable and local concerns during import. Orchestrates auto-populate + validation. |

**Dependencies**: #42 should precede #45. Quick wins (#56, #55, #64) have no dependencies.
**Estimated effort**: 2-3 weeks total.

---

### Phase 4: Version Intelligence (Competitive Differentiator)

_Goal: CrossHook understands version relationships -- the gap no other tool fills._

```
 #41 Trainer/game version correlation ──> #46 Configuration history
                                     ──> #37 Onboarding guidance
```

| Order | Issue                                       | Status | Rationale                                                                                                                                                     |
| ----- | ------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 13    | **#41** -- Trainer/game version correlation | Done   | Highest cross-perspective support (7/8). Detect game updates via Steam manifest changes, warn about trainer compatibility.                                    |
| 14    | **#46** -- Configuration history / rollback | Done   | Builds on version awareness. Track which configs worked, enable diff and rollback.                                                                            |
| 15    | **#37** -- Trainer onboarding guidance      | Done   | With version tracking (#41), health dashboard (#38), and diagnostics (#36) in place, the onboarding flow can guide users through a complete, validated setup. |

**Dependencies**: #41 depends on Steam manifest parsing (exists). #46 builds on #41's version awareness. #37 benefits from #38, #36, and #41 being complete.
**Estimated effort**: 2-3 weeks total.

---

### Phase 5: CLI & Automation (Power Users)

_Goal: CrossHook is usable headlessly, scriptable, and automatable._

```
 #43 CLI completion ──> #44 Offline-first
                   ──> headless launch for Sunshine
```

| Order | Issue                                       | Status | Rationale                                                                                               |
| ----- | ------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------- |
| 16    | **#43** -- CLI completion                   | Done   | Pure wiring to `crosshook-core`. Unlocks scripted usage, automation, Steam Deck console mode.           |
| 17    | **#44** -- Offline-first trainer management | Done   | With CLI complete, ensure all workflows function without network. Critical for Steam Deck portable use. |

**Dependencies**: #43 is standalone wiring work. #44 is a cross-cutting concern that should be validated after CLI exists.
**Estimated effort**: 1-2 weeks total.

---

### Phase 6: Polish & Ecosystem (P2 Features)

_Goal: Enhance the experience for established users. Pick based on demand._

These features have no strict ordering. Prioritize based on community feedback.

| Issue | Category                         | Status | Effort  | Good Pairing With         |
| ----- | -------------------------------- | ------ | ------- | ------------------------- |
| #58   | Extended optimization catalog    | Done   | Low     | #66 (data-driven catalog) |
| #59   | Tap pinning                      | Done   | Low     | #55 (community export)    |
| #54   | Adaptive Deck Mode layout        | Done   | Low     | #47 (pinned profiles)     |
| #50   | Optimization presets             | Done   | Medium  | #58 (extended catalog)    |
| #57   | Custom env vars per profile      | Done   | Low-Med | #58 (extended catalog)    |
| #51   | Gamescope wrapper                | Done   | Low-Med | #58 (extended catalog)    |
| #65   | MangoHud per-profile config      | Done   | Low-Med | #51 (gamescope)           |
| #53   | ProtonDB lookup                  | Done   | Medium  | #41 (version correlation) |
| #52   | Game metadata / cover art        | Done   | Medium  | #53 (ProtonDB)            |
| #60   | Settings expansion               | Done   | Low-Med | Any phase                 |
| #61   | Prefix health monitoring         | Done   | Medium  | #38 (health dashboard)    |
| #62   | Network isolation                | Done   | Low     | #63 (hash verification)   |
| #63   | Trainer hash verification        |        | Low     | #62 (network isolation)   |
| #66   | Data-driven optimization catalog | Done   | Medium  | #58 (extended catalog)    |

**Natural groupings for PRs**:

- **Launch optimization bundle**: #58 + #50 + #57 + #66
- **Steam Deck polish bundle**: #54 + #51 + #65
- **Security bundle**: #62 + #63
- **Ecosystem integration bundle**: #53 + #52
- **Community improvements bundle**: #59 + #55 (if not done earlier)
- **Settings & maintenance bundle**: #60 + #61

### Issue #60 Storage Boundary Note

When planning `#60`, do not assume every new setting belongs in TOML. Evaluate each field against the storage checkpoint:

- user-editable preference -> `settings.toml`
- operational/history/cache metadata -> SQLite metadata DB
- runtime-only values -> in-memory state only

Acceptance criteria for `#60` planning should include explicit persistence rationale plus migration, offline, degraded-mode, and visibility/editability behavior.

### Issue #61 Storage Boundary Note

When planning `#61` (prefix health, disk usage, orphan prefixes, staged-trainer cleanup), do not treat all surfaced data as profile TOML or as throwaway UI state. Evaluate each datum against the storage checkpoint:

- **User-editable preferences** (thresholds, default warning level, whether to show prefix size in profile vs settings) → `settings.toml` where appropriate.
- **Operational / history / cache metadata** (last measured prefix sizes, last scan timestamps, orphan-detection snapshots, cleanup action audit trail, cached free-space checks tied to launch safety) → SQLite metadata DB via existing `MetadataStore` patterns, unless a field is inherently ephemeral.
- **Runtime-only** (in-progress directory walks, live `statvfs` reads during a single session, transient UI loading state) → in-memory only.

Acceptance criteria for `#61` planning and implementation should include:

- Explicit **migration / backward compatibility** for any new metadata rows or keys (including safe behavior on first run and after DB upgrade).
- **Offline / no-network** behavior: prefix and disk features must remain meaningful without remote services (this feature is inherently local filesystem).
- **Degraded behavior** when the metadata DB cannot be opened or written: still allow launch and profile editing where safe; surface a clear, non-blocking warning instead of failing the whole app.
- **User visibility and editability**: what the user can change directly vs what is derived or historical; destructive cleanup actions must be explicit, confirmable, and attributable in the UI.

### Issue #52 Storage Boundary Note

Issue #52's original text says "cache images locally alongside profile TOML files." That wording predates the SQLite metadata DB. Apply the storage checkpoint before implementing:

**Datum classification:**

| Datum                                                       | Layer                                                                                        | Reasoning                                                                                                                             |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Steam Store metadata JSON (name, description, genres, tags) | SQLite `external_cache_entries`                                                              | Payload ~3-15 KiB; fits 512 KiB cap; TTL-based expiry; cache key `steam:appdetails:v1:{app_id}`                                       |
| Cover art / hero image binaries                             | Filesystem `~/.local/share/crosshook/cache/images/`, tracked by new `game_image_cache` table | Images 80 KB-2 MB exceed `MAX_CACHE_PAYLOAD_BYTES`; blobs in SQLite cause WAL pressure; filesystem + DB metadata is the correct split |
| SteamGridDB API key                                         | `settings.toml` (`AppSettingsData.steamgriddb_api_key`)                                      | User-editable preference                                                                                                              |
| Image fetch/display state                                   | Runtime-only (in-memory)                                                                     | Ephemeral UI state                                                                                                                    |

**Implementation requires:**

- New migration (next version): `game_image_cache` table tracking filesystem-stored images (path, checksum, source URL, app ID, expiry, preferred source)
- Reuse `external_cache_entries` for metadata JSON — no schema change needed; mirrors ProtonDB (#53) pattern exactly
- Filesystem image cache at `~/.local/share/crosshook/cache/images/` with `0o700` directory permissions (matches `db.rs` pattern)

**Do NOT** store image binaries in `external_cache_entries` — payloads exceeding `MAX_CACHE_PAYLOAD_BYTES` (512 KiB) silently store `NULL payload_json`, which would break offline art access without any error signal to the caller.

**Persistence/usability summary:**

- **Migration/backward compatibility**: The new migration is additive. Users without it have no cover art but all existing functionality is unaffected.
- **Offline behavior**: Metadata JSON available as stale fallback in `external_cache_entries`. Filesystem images survive offline with no expiry until a new fetch succeeds. Profile cards show cached cover art offline; without cache, cards degrade to text-only.
- **Degraded fallback**: Steam API unavailable -> text-only card, no blocked profile load/launch. SteamGridDB unavailable or unconfigured -> fall back to Steam API art. No art at all -> placeholder or no image region.
- **User visibility/editability**: SteamGridDB API key visible/editable in `settings.toml`. Cached images visible in `~/.local/share/crosshook/cache/images/` and deletable to force re-fetch. Metadata JSON in `external_cache_entries` is an implementation detail.

Acceptance criteria for `#52` planning must include explicit persistence rationale plus migration, offline, degraded-mode, and visibility/editability behavior.

### Issue #62 Storage Boundary Note

When planning `#62` (network isolation for trainers via `unshare --net`), the core deliverable is a per-profile toggle. Apply the storage checkpoint:

**Datum classification:**

| Datum                                        | Layer                                                        | Reasoning                                                                                                         |
| -------------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| Per-profile "Isolate trainer network" toggle | Profile TOML (`network_isolation` field)                     | User-editable per-profile preference; belongs alongside other launch wrapper toggles in the profile config        |
| Isolation active state at launch time        | Runtime-only (in-memory)                                     | Ephemeral; the `unshare --net` wrapper is prepended to the command at launch and not persisted beyond the process |
| Network isolation outcome per launch         | SQLite `launch_operations.diagnostic_json` (existing column) | Launch diagnostics already capture the wrapper chain; no new column or migration needed                           |

**Persistence/usability summary:**

- **Migration/backward compatibility**: The new `network_isolation` field in profile TOML should default to `true` (on). Existing profiles without the field use the default — `serde` default deserialization covers this without a migration.
- **Offline behavior**: Fully local feature; no network dependency. Works identically offline.
- **Degraded behavior**: If `unshare --net` fails at launch time (e.g., unprivileged user namespaces disabled by kernel policy), launch must proceed without isolation and surface a non-blocking warning. The UI toggle remains visible but should be annotated as unavailable when the capability check fails.
- **User visibility/editability**: Toggle visible and editable in the profile editor UI. The setting is stored in the profile TOML file — user-readable and directly editable without CrossHook.

Acceptance criteria for `#62` planning must include explicit persistence rationale plus migration, offline, degraded-mode, and visibility/editability behavior.

---

### Phase 7: Future (P3 Features)

These are tracked but not scheduled. Revisit after Phases 1-6 based on community demand and maintainer capacity.

| Issue                           | Status | Trigger to Revisit                                                               |
| ------------------------------- | ------ | -------------------------------------------------------------------------------- |
| #67 -- Trainer discovery        |        | When community taps reach 50+ profiles and users still struggle to find trainers |
| #68 -- Protontricks integration | Done   | When trainer failure diagnostics (#36) show missing dependencies as a top cause  |
| #69 -- Flatpak distribution     |        | When immutable distro users report AppImage issues                               |
| #70 -- ProtonUp-Qt integration  |        | When Proton migration (#48) shows users lack the needed Proton versions          |
| #71 -- Lutris import            |        | When user acquisition from Lutris becomes a measurable source                    |
| #72 -- Mod management           |        | Only if directly supporting trainer coexistence, not as general mod management   |
| #73 -- Profile collections      |        | When users have 20+ profiles and request organization                            |
| #74 -- Pipeline visualization   |        | When the profile editor feels too complex for new users                          |
| #75 -- Accessibility            |        | When accessibility feedback is received or before a public launch milestone      |
| #76 -- macOS port               |        | When GPTK2 trainer viability is confirmed by community testing                   |
| #77 -- Community Suggestions    | Done   | After #53 (ProtonDB) is live and generating data for pattern extraction          |

---

## Dependency Graph

```
Phase 1 (Foundation)
  #39 Validation errors ─────────────────────────────────────┐
  #40 Dry run ───────────────────────────────────────────────┤
                                                             │
Phase 2 (Diagnostics)                                        │
  #36 Post-launch diagnostics ◄──── #39                      │
  #38 Profile health dashboard                               │
  #49 Diagnostic bundle ◄──── #36 + #38                      │
                                                             │
Phase 3 (Profiles)                                           │
  #56 Clone (quick win)                                      │
  #55 Community export (quick win)                           │
  #64 Stale launchers (quick win)                            │
  #47 Pinned profiles                                        │
  #48 Proton migration ◄──── #38                             │
  #42 Override layers                                        │
  #45 Import wizard ◄──── #42                                │
                                                             │
Phase 4 (Version Intelligence)                               │
  #41 Version correlation                                    │
  #46 Config history ◄──── #41                               │
  #37 Onboarding ◄──── #38 + #36 + #41                      │
                                                             │
Phase 5 (CLI)                                                │
  #43 CLI completion                                         │
  #44 Offline-first ◄──── #43                                │
                                                             │
Phase 6 (Polish) ── pick based on demand ────────────────────┘
  #58 Extended catalog     #50 Presets        #62 Net isolation
  #59 Tap pinning          #57 Custom env     #63 Hash verify
  #54 Deck layout          #51 Gamescope      #66 Data catalog
  #53 ProtonDB             #65 MangoHud       #60 Settings
  #52 Cover art            #61 Prefix health
```

---

## Effort Estimates by Phase

| Phase                  | Issues | Status  | Estimated Total | Cumulative |
| ---------------------- | ------ | ------- | --------------- | ---------- |
| Quick Wins             | 7      | Done    | 2-3 days        | 2-3 days   |
| Phase 1: Foundation    | 2      | Done    | 2-3 days        | ~1 week    |
| Phase 2: Diagnostics   | 3      | Done    | 1-2 weeks       | ~3 weeks   |
| Phase 3: Profiles      | 7      | Done    | 2-3 weeks       | ~6 weeks   |
| Phase 4: Version Intel | 3      | Done    | 2-3 weeks       | ~9 weeks   |
| Phase 5: CLI           | 2      | Done    | 1-2 weeks       | ~11 weeks  |
| Phase 6: Polish        | 17     | Partial | Pick & choose   | Ongoing    |
| Phase 7: Future        | 11     |         | Not scheduled   | Backlog    |

---

## Anti-Pattern Checklist

Before starting any feature, verify it does not fall into a warned pattern:

- Does this make **trainer management** better, or does it make CrossHook more like Lutris?
- Is the maintenance burden proportional to the user value?
- Does it work **offline** on Steam Deck?
- Does it respect user **privacy** (no telemetry, no accounts, no tracking)?
- Would a **solo maintainer** be able to keep this working for 3 years?
- Is this the **simplest** solution, or am I over-engineering?
- Is the storage boundary explicit (TOML vs SQLite metadata vs runtime state), with a clear reason?
- Have persistence/usability expectations been written (migration, offline, degraded mode, visibility/editability)?

If any answer raises doubt, reconsider or scope down before implementing.
