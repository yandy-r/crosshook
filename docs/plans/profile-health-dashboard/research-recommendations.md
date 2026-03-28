# Profile Health Dashboard: Recommendations & Risk Assessment (v2)

**Feature**: Profile health dashboard with staleness detection (GitHub #38)
**Phase**: 2 (Diagnostics & Health), Order #4
**Dependencies**: #39 (actionable validation errors) -- DONE
**Downstream**: #49 (diagnostic bundle export), #48 (Proton migration tool), #64 (stale launcher detection)
**Last updated**: 2026-03-28
**Research team**: Synthesized from API research, business analysis, technical design, UX research, security evaluation, engineering practices review.
**Revision**: v2 -- revised for SQLite metadata layer (PRs 89-91, migration v1-v5).

---

## Executive Summary

The profile health dashboard can be built almost entirely on existing infrastructure. Domain complexity is **LOW** -- approximately 80% of the logic already exists in `validate_all()`, `ValidationError::help()`, `ProfileStore::list()`, and `ProfileStore::load()`. The `CompatibilityBadge` component provides a proven badge pattern for tiered health status.

**What changed in v2**: The SQLite metadata layer (PRs 89-91) adds three capabilities that the original spec could not leverage:
1. **Launch history enrichment** -- `query_failure_trends(days)` and `query_last_success_per_profile()` are already implemented and require only a Tauri command wrapper to surface failure trend badges alongside filesystem health.
2. **Launcher drift detection** -- `launcher_sync` already tracks `drift_state` per launcher. A profile with a drifted launcher can surface a "launcher out of sync" health issue at near-zero implementation cost.
3. **Optional health persistence** -- health results *can* now be persisted for trend tracking, but this is deferred to Phase D to avoid over-engineering Phase A.

The primary new work remains: (1) a Rust-side `GameProfile` path validation function, (2) a new `ProfileHealthInfo` struct in `profile/health.rs`, (3) a `profile_health_check_all` Tauri command, and (4) a frontend health badge component. Metadata enrichment layers on top as additive, optional composition.

**Key v2 decision**: Phase A remains pure-filesystem health checking. Metadata enrichment (failure trends, last success, launcher drift) is a separate Phase B composition step. This preserves the original KISS architecture while opening the door to richer health signals.

---

## Implementation Recommendations

### Technical Approach

#### Key Architectural Decisions (Synthesized from all 6 researchers)

| Decision | Recommendation | Rationale | v2 Change |
| --- | --- | --- | --- |
| **New module vs. extend existing** | New `profile/health.rs` file (not top-level `health/` module) | Health checks filesystem paths at rest; launch validation checks runtime constraints. One new file is sufficient. | No change |
| **Sync vs. async batch** | Start synchronous via `spawn_blocking` | 50 profiles x 8 paths x ~1ms per `Path::exists()` = ~400ms worst case. Acceptable for synchronous `invoke`. | No change |
| **Health status granularity** | Tri-state display: Healthy / Stale / Broken | Aligns with issue #38 spec and `LauncherInfo::is_stale` precedent. Security advisory: internally distinguish `Missing` (ENOENT) from `Inaccessible` (EACCES). | No change |
| **Caching** | No caching (always re-check) | Filesystem state changes at any time. Check is fast enough. Display "last checked at" timestamp. | No change |
| **Health issue type** | Reuse `LaunchValidationIssue` (not new type) | Practices-researcher pushed back on new `HealthIssue` type: existing `help` text is already remediation guidance. | No change |
| **Startup validation** | Always-on async, non-blocking, passive | Spawn async task after UI renders. Emit `profile-health-batch-complete` Tauri event. No modal, no blocking. | No change |
| **Batch concurrency** | Sequential or bounded (4 concurrent) | Avoid I/O pressure on slow storage (SD card). Sequential is simpler and fast enough. | No change |
| **Health persistence** | **NEW** -- Ephemeral in Phase A; optional SQLite persistence in Phase D | Original spec (Business Rule #8) said "never written to disk." With MetadataStore available, this is unnecessarily restrictive. Relax to: ephemeral for now, optional persistence later for trend tracking. | **Changed from v1** |
| **Metadata enrichment** | **NEW** -- Additive composition layer, not woven into core health check | Failure trends, last success, launcher drift are separate data sources. Compose them in the Tauri command layer, not inside `profile/health.rs`. Keeps health module pure-filesystem. | **New in v2** |
| **MetadataStore dependency** | **NEW** -- Fail-soft optional | Health checks must work when MetadataStore is disabled. Metadata enrichment returns empty/default data when unavailable, matching existing `with_conn` pattern. | **New in v2** |

**Critical correctness requirement** (unchanged from v1): The `steam_applaunch` validation requires `steam_client_install_path` from `AppSettings`. The `derive_steam_client_install_path()` helper must move from `src-tauri/src/commands/profile.rs` into `crosshook-core`.

#### Core Architecture: Profile-to-Validation Bridge (Unchanged)

The main gap is that `validate_all()` operates on `LaunchRequest`, not `GameProfile`. The conversion currently happens implicitly in the frontend (`useProfile.ts::normalizeProfileForEdit`). The health dashboard needs this in Rust.

**Recommended approach**: Add a `GameProfile::to_launch_request()` method to `crosshook-core/src/profile/models.rs`. Use `resolve_launch_method(profile)` first, then check only method-relevant paths.

```
GameProfile -> resolve_launch_method() -> to_launch_request() -> validate_all() -> Vec<LaunchValidationIssue>
                                                                                         |
                                                                                         v
                                                                        derive ProfileHealthStatus from severity
```

#### NEW: Metadata Enrichment Architecture (v2)

The metadata enrichment is a **composition layer** that runs after the core filesystem health check:

```
Phase A (core):
  GameProfile -> path validation -> ProfileHealthResult (filesystem-only)

Phase B (enrichment, additive):
  ProfileHealthResult + MetadataStore ->
    + query_failure_trends(7)     -> failure trend badge per profile
    + query_last_success_per_profile() -> "last success: 3 days ago" annotation
    + launcher drift state query  -> "launcher out of sync" issue
  -> EnrichedProfileHealthResult
```

The enrichment step lives in the Tauri command handler (`commands/profile.rs`), not in `crosshook-core/src/profile/health.rs`. This keeps the health module independent of the metadata layer and maintains testability with `tempfile::tempdir()`.

#### Batch Validation Command (Revised)

Expose via Tauri as `profile_health_check_all`:

1. Call `ProfileStore::list()` to get all profile names
2. For each name, call `ProfileStore::load()` (catch `TomlDe` errors per-profile, report as "Broken")
3. Call `profile.to_launch_request()` then `validate_all()`
4. Collect results into `Vec<ProfileHealthInfo>`
5. **NEW (Phase B)**: Optionally enrich with `MetadataStore` queries (failure trends, last success, launcher drift)
6. Return to frontend

Also expose `profile_health_check(name)` for single-profile checks.

**Path utility promotion**: The private functions `require_directory()`, `require_executable_file()`, and `is_executable_file()` in `request.rs` should be promoted to `pub(crate)`.

**`derive_steam_client_install_path()` relocation**: Must move to `crosshook-core`. This is the only substantive code migration needed beyond the new health module.

#### Frontend Integration (Unchanged)

Reuse `CompatibilityBadge` CSS class pattern for health status chips. Add a `ProfileHealthSummary` component with aggregate counts and per-profile expandable detail.

### Phasing: What to Build First (REVISED for v2)

**Phase A (MVP -- 1-2 days)** -- *Unchanged scope*:

1. `GameProfile::to_launch_request()` in `crosshook-core`
2. `profile_health_check_all` Tauri command
3. Simple frontend list showing health results with badge per profile

**Phase B (Polish + Metadata Enrichment -- 2-3 days)** -- *Expanded from v1*:

1. On-demand "Check Health" button in the profile sidebar
2. Per-issue remediation hints (reuse `ValidationError::help()` text)
3. Filter/sort by health status
4. **NEW**: Failure trend badges via `query_failure_trends(7)` Tauri command
5. **NEW**: "Last successful launch" annotation via `query_last_success_per_profile()`
6. **NEW**: Launcher drift issues via launcher `drift_state` query
7. **NEW**: Collection-scoped health summaries (health check filtered by collection membership)

**Phase C (Startup integration -- 0.5 days)** -- *Revised for v2*:

1. Non-blocking startup health check via Tauri event, don't block UI init
2. Emit `profile-health-batch-complete` event; passive badges appear when user opens profile list
3. Startup summary banner for broken profiles only (non-blocking, non-modal)

**Tech-designer insight (v2)**: If Phase D ships before or alongside Phase C, cached health snapshots enable **instant startup badge rendering** -- show last-known health badges immediately from SQLite on app open, then refresh async in background. This eliminates the loading spinner entirely for returning users. Without Phase D, startup badges require a full filesystem scan before any badges appear. This is a strong argument for pulling the Phase D persistence table forward if startup UX is a priority. Consider reordering: Phase D (persistence) before Phase C (startup) if instant startup display is valued.

**Phase D (Persistence + Trends -- 1-2 days)** -- *New phase in v2*:

1. Add `profile_health_snapshots` table (migration v6)
2. Persist health check results on each batch validation
3. Expose trend arrows in UI ("profile got worse/better since last check")
4. Historical health comparison ("7 days ago: 2 broken, today: 0 broken")

### Quick Wins

#### Existing Quick Wins (Unchanged from v1)

| Win | Effort | Rationale |
| --- | --- | --- |
| Reuse `CompatibilityBadge` for health chips | Minutes | Same CSS class pattern, same tiered colors |
| Reuse `CollapsibleSection` for detail panels | Minutes | Already used in LaunchPanel and CompatibilityViewer |
| Reuse `ValidationError::help()` for remediation text | Zero | Already written for all 20+ validation error variants |
| Reuse `sanitize_display_path()` for path display | Minutes | Already strips `$HOME` to `~` |

#### NEW Quick Wins Enabled by Metadata Layer (v2)

| Win | Effort | Rationale |
| --- | --- | --- |
| **Failure trend badges** | Low (~1 Tauri command wrapper) | `query_failure_trends(days)` already implemented in `MetadataStore`. Returns per-profile success/failure counts. Just expose via Tauri command and display as badge annotation. |
| **"Last successful launch" annotation** | Low (~1 Tauri command wrapper) | `query_last_success_per_profile()` already returns ISO timestamps. Display as "Last success: 3 days ago" in health detail view. |
| **Launcher drift detection** | Low (~1 SQL query) | `launchers.drift_state` column already populated by `launcher_sync`. Query `SELECT drift_state FROM launchers WHERE profile_id = ?1` during health enrichment. |
| **Collection-scoped health** | Low (composition) | `list_profiles_in_collection()` returns profile names. Filter `batch_check_health()` input to collection members. Zero new infrastructure. |
| **Favorites-only health check** | Low (composition) | `list_favorite_profiles()` returns profile names. Same composition pattern as collections. |
| **"Most problematic" ranking** | Trivial | Combine failure trends + filesystem health to sort profiles by "needs attention" priority. Pure frontend logic on existing data. |

### Leveraging Existing validate() Infrastructure (Unchanged)

The `validate_all()` function (`request.rs:442`) is purpose-built for this. Method-specific collectors: `collect_steam_issues()`, `collect_proton_issues()`, `collect_native_issues()`.

What `validate_all()` does NOT check (and health dashboard should add):
- Trainer paths when `launch_game_only = true`
- Icon path existence (`launcher.icon_path`)
- DLL injection paths (`InjectionSection.dll_paths`)
- Whether Proton version is still installed
- Profile TOML parse errors

### Edge Cases Requiring Special Handling (Revised)

1. **Empty/unconfigured profiles**: Introduce "Unconfigured" state for profiles where `game.executable_path` is empty. *Unchanged.*

2. **Community-imported profiles**: Show "imported -- use Auto-Populate" instead of generic "Broken." *Unchanged.*

3. **Removable media (SD card)**: Configured path + `Path::exists() == false` = always **Stale**. *Unchanged.*

4. **Proton auto-updates**: Detect Proton path pattern for targeted remediation. *Unchanged.*

5. **Remediation text context**: Reuse `ValidationError::help()` verbatim for Phase A. *Unchanged.*

6. **Missing vs. inaccessible paths**: Use `std::fs::metadata()` to distinguish. *Unchanged.*

7. **`steam_client_install_path` injection**: Must call `derive_steam_client_install_path()`. *Unchanged.*

8. **NEW: MetadataStore unavailable during health check**: If `MetadataStore.available == false`, the enrichment step returns empty data. Health check still works with filesystem-only results. The health module must not depend on metadata availability for core functionality. Mirror the existing `with_conn` fail-soft pattern.

9. **NEW: Launch history for renamed profiles**: `launch_operations` records `profile_name` at launch time. If a profile was renamed, historical launch data may reference the old name. `profile_id` (stable UUID) links them, but the health enrichment query should join on `profile_id` via `lookup_profile_id()`, not on `profile_name`. This ensures failure trends survive renames.

10. **NEW: Launcher drift for profiles without exported launchers**: Not all profiles have exported launchers. The drift check should silently return "no launcher" rather than "missing launcher" -- the absence of an exported launcher is not a health issue.

---

## Improvement Ideas

### Related Features This Enables (Revised)

| Feature | Issue | How Health Dashboard Helps | v2 Change |
| --- | --- | --- | --- |
| Diagnostic bundle export | #49 | Health report becomes a section in the bundle. **NEW**: Launch history summary (failure trends, last success) can also be included as a separate section. | **Expanded** |
| Proton migration tool | #48 | Health check identifies "Proton path missing" as the trigger. **NEW**: `query_failure_trends()` can flag profiles that have been failing since a Proton update. | **Expanded** |
| Stale launcher detection | #64 | **REVISED**: `launcher_sync` already tracks `drift_state`. Health dashboard surfaces this directly rather than computing `LauncherInfo::is_stale` separately. #64 may be partially addressed by Phase B metadata enrichment. | **Revised** |
| Import wizard | #45 | Health check validates imported profiles immediately. *Unchanged.* | No change |
| **NEW: Profile quality score** | -- | Combine filesystem health + launch success rate + launcher alignment into a composite quality metric. Natural extension of Phase B enrichment. | **New** |
| **NEW: Collection health reports** | -- | "All profiles in my Steam Deck collection are healthy" -- useful for pre-session verification. | **New** |

### Future Enhancements (Revised)

1. **Auto-repair for common issues**: When Proton path is missing but newer version exists, offer one-click update. *Unchanged.*

2. **File watching (inotify)**: Over-engineering for v1. *Unchanged -- still not recommended.*

3. **Health history / trend** (REVISED): Originally listed as "over-engineering for Phase 1." With MetadataStore available, this is now **feasible in Phase D** via a `profile_health_snapshots` table. The schema is straightforward: `(snapshot_id, profile_id, status, issue_count, checked_at)`. Trend arrows ("got better/worse") require only comparing latest two snapshots per profile.

4. **Batch repair**: "Fix all stale Proton paths" button. *Unchanged.*

5. **Profile quality score**: Beyond binary health, combine filesystem health + launch success rate + launcher alignment. *Feasible as Phase B+ extension.*

6. **NEW: Health export as JSON**: With MetadataStore persistence (Phase D), health history can be exported as JSON for external tooling or community sharing. Extends #49 diagnostic bundle.

7. **NEW: CLI health command**: `crosshook health` in `crosshook-cli`. Since `check_profile_health()` and `batch_check_health()` live in `crosshook-core`, this is trivial to wire. The metadata enrichment step would also be available since `MetadataStore` can be opened from CLI context.

---

## Risk Assessment

### Technical Risks (Revised)

| Risk | Likelihood | Impact | Mitigation | v2 Change |
| --- | --- | --- | --- | --- |
| `GameProfile -> LaunchRequest` conversion logic diverges from frontend normalization | Medium | High -- false health results | Write comprehensive tests comparing Rust conversion output with known-good frontend normalization. | No change |
| `steam_client_install_path` not injected into health check `LaunchRequest` | High | High -- all Steam profiles show false "Broken" | Relocate `derive_steam_client_install_path()` to `crosshook-core`. **Blocking for MVP.** | No change |
| Batch validation I/O blocks Tauri main thread | Low | Medium -- UI freezes | Start synchronous (~400ms acceptable). Use `spawn_blocking` only if needed. | No change |
| Profile TOML parse errors crash batch validation | Medium | Medium -- one bad profile breaks all results | Catch `ProfileStoreError::TomlDe` per profile, report as "Broken (parse error)" | No change |
| Health check reports false "broken" for removable media | Medium | Low -- user confusion | Hardened rule: configured path + missing = always **Stale**, never **Broken**. | No change |
| `validate_all()` internal changes break health dashboard | Low | Medium | Health module depends on `validate_all()` public API, which is stable. | No change |
| Empty profiles classified as "Broken" alarm new users | Medium | Medium -- bad first impression | Introduce "Unconfigured" state. | No change |
| Community-imported profiles appear immediately broken | Medium | Low -- expected but jarring | Show targeted "use Auto-Populate" message. | No change |
| **NEW: MetadataStore integration complexity** | Low | Medium -- enrichment fails silently | Follow existing fail-soft pattern (`with_conn` returns default when unavailable). Health check works without metadata. Enrichment is additive, never blocking. | **New in v2** |
| **NEW: Composite health scoring ambiguity** | Medium | Medium -- user confusion | If filesystem says "Healthy" but launch history shows 100% failure rate, what badge do we show? **Recommendation**: Keep separate indicators in Phase B. Do not combine into single score until user feedback validates the need. | **New in v2** |
| **NEW: Migration coupling (v6)** | Low | Low -- delays Phase D only | Health persistence (Phase D) requires migration v6. Decouple from Phase A entirely. Phase A has zero migration dependency. | **New in v2** |
| **NEW: Launch history data for renamed profiles** | Low | Low -- incomplete trend data | `launch_operations` stores both `profile_name` and `profile_id`. Enrichment queries should join on `profile_id` (stable UUID) via `lookup_profile_id()`, not on `profile_name`. | **New in v2** |

### Removed Risks (Addressed by SQLite Implementation)

| Original Risk | Why Removed |
| --- | --- |
| "Health history/trend is over-engineering" | MetadataStore provides the persistence infrastructure. Trend tracking is now a straightforward Phase D extension, not a major new system. |
| "No way to track launch success/failure patterns" | `launch_operations` table with `query_failure_trends()` and `query_last_success_per_profile()` already exist. |
| "Launcher staleness must be computed separately" | `launcher_sync` already populates `drift_state`. Direct query replaces separate computation. |
| "Profile identity is filename-dependent" | Stable UUIDs via `profiles.profile_id` decouple identity from filenames. Health enrichment survives renames. |

### Security Risks (Revised from security researcher)

| ID | Severity | Risk | Mitigation | v2 Change |
| --- | --- | --- | --- | --- |
| W-1 | Warning | **CSP disabled** (`tauri.conf.json:23`, `"csp": null`) -- new IPC commands increase exposure | Enable CSP before shipping health dashboard. **More urgent in v2**: health dashboard adds 2-4 new Tauri commands. | **Elevated priority** |
| W-2 | Warning | **Raw paths in IPC responses** -- health check returns filesystem paths to frontend | Apply `sanitize_display_path()` to all path fields before returning over IPC. | No change |
| W-3 | Warning | **Diagnostic bundle path leak** (#49 downstream) | Sanitize all paths in exported health JSON. | No change |
| W-4 | Warning | **NEW: Paths in persisted health snapshots** (Phase D) | If health results are persisted to SQLite, apply `sanitize_display_path()` before persistence, not just before IPC. New surface not covered by original W-2. | **New in v2** |
| W-5 | Warning | **NEW: Launcher paths in drift reports** | `launchers.script_path` and `launchers.desktop_entry_path` are full paths. Sanitize before surfacing in health enrichment. | **New in v2** |
| A-1 | Advisory | **ENOENT vs EACCES conflation** | Use `std::fs::metadata()` to distinguish. | No change |
| A-3 | Advisory | **TOCTOU** -- path could disappear between check and launch | Accepted risk. Health check is informational. | No change |
| A-4 | Advisory | **Batch I/O on slow storage** | Sequential validation, bound to 4 concurrent if parallelized. | No change |
| A-6 | Advisory | **NEW: Soft-deleted profiles in collection-scoped health** | Collection health queries must filter `deleted_at IS NOT NULL` profiles. Use existing join pattern from `list_profiles_in_collection()` which already has this filter. | **New in v2** |

### Performance Risks (Revised)

- **Profile count scaling**: `ProfileStore::list()` + `load()` for each profile is O(n) filesystem reads. ~400ms for 50 profiles on desktop, up to 2s on Steam Deck SD card. *Unchanged.*
- **NEW: Metadata enrichment adds database round-trips**: `query_failure_trends()` and `query_last_success_per_profile()` are SQL aggregate queries. For typical profile counts (<50), this adds ~10-50ms. The queries use existing indexes (`idx_launch_ops_profile_id`, `idx_launch_ops_started_at`). Acceptable overhead.
- **NEW: Collection-scoped health is cheaper**: Filtering to a collection (typically 5-15 profiles) reduces both filesystem and database load proportionally.
- **NEW: Cached startup rendering eliminates loading spinner**: If health snapshots are persisted (Phase D), startup can display last-known badges instantly from SQLite, then refresh async in background. This turns the 400ms-2s filesystem scan from a blocking UX cost into a zero-latency cached display with eventual consistency. (Tech-designer finding)
- **Recommendation**: Start with on-demand only (button click). Add optional startup check in Phase C after measuring real-world performance.

### UX Risks (Revised)

- **Warning fatigue**: Show aggregate summary, not wall of alerts. *Unchanged.*
- **Stale-after-fix confusion**: Invalidate health results on profile save. *Unchanged.*
- **NEW: Metadata enrichment information density**: Adding failure trend badges, last success timestamps, and launcher drift indicators alongside filesystem health badges risks visual clutter. **Recommendation**: Use progressive disclosure -- show filesystem health badge inline, expand to show metadata enrichment on click/expand. Do not show all enrichment data by default.
- **NEW: "Healthy but always fails" paradox**: A profile with all paths present (filesystem-healthy) but 100% launch failure rate needs careful UX. **Recommendation**: Show filesystem badge as "Healthy" but add a small warning icon with tooltip "Recent launches have failed -- check launch logs." Do not change the filesystem health badge based on launch history.

---

## Alternative Approaches

### UI Placement (Resolved: Option B -- Unchanged)

Inline badges in profile selector. Zero navigation changes; health visible in context.

### Batch Validation Strategy (Resolved: Option A -- Unchanged)

Synchronous `validate_all()` via `spawn_blocking`. Simplest implementation.

### Startup Behavior (Resolved: Always-on Async -- Unchanged)

Spawn async ~500ms after UI renders. Emit `profile-health-batch-complete` Tauri event.

### NEW: Metadata Enrichment Strategy (v2)

| Option | Approach | Verdict | Rationale |
| --- | --- | --- | --- |
| **A** | Weave metadata queries into `profile/health.rs` | Rejected | Couples health module to MetadataStore. Breaks testability with `tempfile::tempdir()`. Violates separation of concerns. |
| **B** | Compose in Tauri command handler | **Selected** | Health module stays pure-filesystem. Tauri command calls `batch_check_health()`, then enriches results with MetadataStore queries. Fail-soft: if metadata unavailable, return un-enriched results. Matches existing integration pattern. |
| **C** | Separate enrichment Tauri command | Acceptable alternative | Frontend calls `batch_validate_profiles` then `enrich_health_with_metadata` separately. More round-trips but cleaner separation. Consider if Phase B becomes complex. |

### NEW: Health Persistence Strategy (v2)

| Option | Approach | Verdict | Rationale |
| --- | --- | --- | --- |
| **A** | Never persist (original v1 Business Rule #8) | Rejected (for v2) | Unnecessarily restrictive now that MetadataStore exists. |
| **B** | Persist every check to `profile_health_snapshots` table | **Selected for Phase D** | Enables trend tracking, "got better/worse" arrows, historical comparison. Schema: `(snapshot_id, profile_id, status, issue_count, checked_at)`. Migration v6. **Additional benefit** (tech-designer): enables instant startup badge rendering from cached last-known state, eliminating loading spinner for returning users. |
| **C** | Persist only when status changes | Deferred | More complex (requires comparison with previous state). Consider if storage becomes a concern. |
| **D** | Cache in MetadataStore, no history | Rejected | Loses trend data. Cache invalidation is harder than re-computation. |

### NEW: Persistence Table vs. Projection from Existing Tables (v2, tech-designer trade-off)

| Option | Approach | Verdict | Rationale |
| --- | --- | --- | --- |
| **A** | Dedicated `profile_health_snapshots` table | **Selected** | Clean separation. Simple queries. Can store metadata not derivable from other tables (filesystem check results). Path health (do files exist?) is fundamentally different from launch history (did launch succeed?). A path can exist but launch can fail (optimization conflict), and a path can be missing but last launch succeeded (deleted after launch). |
| **B** | No new table; health as projection from `launch_operations` + filesystem | Rejected | Cannot persist filesystem check results. Cannot show "last known health" without re-running checks. Loses the instant startup rendering benefit. |

---

## Task Breakdown Preview

### Pre-ship Security Items (Unchanged)

| Task | File(s) | Complexity | Blocking? |
| --- | --- | --- | --- |
| Enable CSP in Tauri config (W-1) | `src-tauri/tauri.conf.json` | Low | Yes -- do before shipping any new IPC surface |
| Apply `sanitize_display_path()` to all health report path fields (W-2) | `src-tauri/src/commands/profile.rs` | Trivial | Yes -- required for health check command |

### Phase A: Core Health Check (MVP) -- Locked Scope

_Unchanged from v1. 12 tasks, pure filesystem health checking. No metadata dependency._

| # | Task | File(s) | Complexity | Dependencies |
| --- | --- | --- | --- | --- |
| 1 | Relocate `derive_steam_client_install_path()` to `crosshook-core` | `src-tauri/src/commands/profile.rs` -> `crates/crosshook-core/src/profile/models.rs` (or `health.rs`) | Low | None -- **blocking for all Steam profile health checks** |
| 2 | Promote `require_directory()`, `require_executable_file()`, `is_executable_file()` to `pub(crate)` | `crates/crosshook-core/src/launch/request.rs` | Trivial | None |
| 3 | Add `GameProfile::to_launch_request()` conversion method | `crates/crosshook-core/src/profile/models.rs` | Medium | Task 1 |
| 4 | Create `ProfileHealthStatus` enum + `ProfileHealthInfo` struct | New: `crates/crosshook-core/src/profile/health.rs` | Low | Task 3 |
| 5 | Implement `check_profile_health()` and `batch_check_health()` with "Unconfigured" detection | `crates/crosshook-core/src/profile/health.rs` | Medium | Task 4 |
| 6 | Write Rust unit tests (use `tempfile::tempdir()` + `ProfileStore::with_base_path()` pattern) | `crates/crosshook-core/src/profile/health.rs` | Medium | Task 5 |
| 7 | Add `profile_health_check_all` and `profile_health_check` Tauri commands (sanitize paths) | `src-tauri/src/commands/profile.rs` | Low | Task 5, pre-ship W-2 |
| 8 | Create `ProfileHealthBadge` component (reuse `crosshook-status-chip` CSS) | New: `src/components/ProfileHealthBadge.tsx` | Low | None |
| 9 | Create `useProfileHealth` hook (mirrors `useLaunchState` reducer pattern) | New: `src/hooks/useProfileHealth.ts` | Medium | Task 7 |
| 10 | Add inline health badges to profile selector list | `src/components/ProfileEditor.tsx` | Low | Tasks 8-9 |
| 11 | Add per-issue remediation hints (reuse `ValidationError::help()` text verbatim) | `src/components/ProfileEditor.tsx` | Low | Tasks 9-10 |
| 12 | Invalidate/re-check health when any profile is saved | `src/hooks/useProfileHealth.ts` | Low | Task 9 |

**Change count**: 1 new Rust file, 1 relocated function, 1 new Tauri command, 1 new hook, 1 new component, 3-4 modified files.
**Estimated complexity**: Medium (3-5 days)

### Phase A Explicit Deferrals

These are NOT in Phase A scope:
- DLL injection path checking (Phase B)
- Icon path validation (Phase B)
- Filter/sort profiles by health status (Phase B)
- **Metadata enrichment (Phase B)** -- failure trends, last success, launcher drift
- **Health persistence (Phase D)** -- no migration v6 in Phase A
- File watching / `notify` crate (not planned)
- Background polling timer (not planned)
- Auto-repair / batch fix (Phase D / #48)
- Composite health scoring (Phase B+, only if user feedback validates need)

### Phase B: Detail, Remediation UI, and Metadata Enrichment (REVISED)

| Task | Complexity | Dependencies | v2 Change |
| --- | --- | --- | --- |
| Add health detail section to `ProfileEditor` with `CollapsibleSection` | Low | Phase A | No change |
| Add "Check Health" button to profile sidebar/toolbar | Low | Phase A | No change |
| Add path-specific checks: DLL injection paths, icon path, Proton version | Medium | Phase A | No change |
| Add filter/sort profiles by health status | Low | Phase A | No change |
| Distinguish ENOENT vs EACCES for remediation text (security A-1) | Low | Phase A | No change |
| **NEW**: Add `query_failure_trends_for_health` Tauri command wrapping `MetadataStore::query_failure_trends(7)` | Low | Phase A + MetadataStore | **New** |
| **NEW**: Add `query_last_success_for_health` Tauri command wrapping `MetadataStore::query_last_success_per_profile()` | Low | Phase A + MetadataStore | **New** |
| **NEW**: Add launcher drift query for health enrichment (query `launchers.drift_state` by profile) | Low | Phase A + MetadataStore | **New** |
| **NEW**: Compose metadata enrichment in Tauri command handler (fail-soft) | Medium | Above 3 tasks | **New** |
| **NEW**: Add failure trend badge annotation to health detail view | Low | Enrichment Tauri command | **New** |
| **NEW**: Add "Last successful launch" annotation to health detail view | Low | Enrichment Tauri command | **New** |
| **NEW**: Add collection-scoped health check (filter by collection membership) | Low | Phase A + collections | **New** |
| **NEW**: Add favorites-only health check option | Low | Phase A + favorites | **New** |

**Estimated complexity**: Medium (2-4 days, expanded from 1-2 days in v1)

### Phase C: Startup Integration (REVISED -- benefits from Phase D persistence)

| Task | Complexity | Dependencies |
| --- | --- | --- |
| Implement always-on non-blocking startup health check via Tauri event | Medium | Phase A |
| Emit `profile-health-batch-complete` event; passive badges appear when user opens profile list | Low | Phase A |
| Add startup summary banner for broken profiles only (non-blocking, non-modal) | Low | Phase A |

**Estimated complexity**: Low (0.5-1 day)

**Tech-designer insight (v2)**: If Phase D ships before or alongside Phase C, cached health snapshots enable **instant startup badge rendering** -- show last-known health badges immediately from SQLite on app open, then refresh async in background. This eliminates the loading spinner entirely for returning users. Without Phase D, startup badges require a full filesystem scan before any badges appear. This is a strong argument for pulling the Phase D persistence table forward if startup UX is a priority. Consider reordering: Phase D (persistence) before Phase C (startup) if instant startup display is valued.

### Phase D: Persistence, Trends, and Downstream Integration (NEW + Revised)

| Task | Complexity | Dependencies | v2 Change |
| --- | --- | --- | --- |
| **NEW**: Design `profile_health_snapshots` table schema | Low | Phase A | **New** |
| **NEW**: Add migration v6 for health snapshots table | Low | Schema design | **New** |
| **NEW**: Persist health check results after each batch validation | Medium | Migration v6 | **New** |
| **NEW**: Add trend comparison query (latest vs. previous snapshot per profile) | Medium | Persistence | **New** |
| **NEW**: Add trend arrows in UI ("profile got worse/better") | Low | Trend query | **New** |
| **NEW**: Add historical health summary view ("7 days ago vs. today") | Medium | Trend query | **New** |
| Export health report as JSON section in diagnostic bundle (#49) -- sanitize paths (W-3) | Low | Phase A, #49 | No change |
| Use health check results to trigger Proton migration flow (#48). **Enhanced**: failure trend data can identify profiles failing since Proton update. | Medium | Phase A, #48 | **Enhanced** |
| Populate launcher drift from health check results (#64). **REVISED**: `launcher_sync.drift_state` already exists; #64 may be partially addressed by Phase B enrichment. | Low | Phase A | **Revised** |

**Estimated complexity**: Medium-High (3-5 days, new phase)

---

## Key Decisions

### Resolved by Team Consensus (Unchanged from v1)

| # | Decision | Resolution | Resolved By |
| --- | --- | --- | --- |
| 1 | **Frontend placement** | Inline badges (Option B) | UX researcher, business analyzer |
| 2 | **Startup behavior** | Always-on async, non-blocking, passive | Business analyzer, UX researcher, tech designer |
| 3 | **Issue type design** | Reuse `LaunchValidationIssue` | Practices researcher |
| 4 | **Removable media policy** | Missing = always Stale, never Broken | Business analyzer |
| 5 | **Unconfigured profile handling** | Introduce "Unconfigured" state | Business analyzer, UX researcher |
| 6 | **Module placement** | `profile/health.rs` (not top-level module) | Practices researcher |
| 7 | **Batch strategy** | Synchronous via `spawn_blocking` | API researcher |

### NEW Decisions for v2

| # | Decision | Resolution | Rationale |
| --- | --- | --- | --- |
| 8 | **Metadata enrichment location** | Tauri command handler, not health module | Keeps `profile/health.rs` pure-filesystem and testable with tempdir. Enrichment is composition, not coupling. |
| 9 | **Health persistence timing** | Phase D, not Phase A | Re-computation (<500ms) is cheaper than cache invalidation. Defer until trend tracking is needed. |
| 10 | **Composite health scoring** | Separate indicators, not combined score | Combining filesystem health + launch history + launcher drift into one badge creates UX ambiguity. Keep distinct until user feedback validates composition. |
| 11 | **Business Rule #8 revision** | "Ephemeral in Phase A, optional persistence in Phase D" | Original "never written to disk" is unnecessarily restrictive with MetadataStore available. |
| 12 | **Failure trends time window** | Default 7 days | Balances recency vs. noise. Configurable via Tauri command parameter. |
| 13 | **Health persistence table vs. projection** | New `profile_health_snapshots` table (not derived from existing tables) | Path health and launch history are fundamentally different data. Cannot project one from the other. (Tech-designer trade-off analysis) |

### Still Open

1. **Health status labels**: Use "Healthy / Needs Attention / Cannot Launch" or "Healthy / Stale / Broken"? *Unchanged from v1.*

2. **Proton version staleness**: When Steam updates Proton, is the old profile "stale" or "broken"? Per hardened rule, this is **Stale** (missing path). *Unchanged from v1.*

3. **NEW: Enrichment data display density**: How much metadata enrichment should be visible by default vs. on expand? Risk of visual clutter. Needs UX validation during Phase B implementation.

4. **NEW: Collection health check scope for startup banner**: Should the startup health banner count all profiles or only favorites/pinned? With collections available, there's a question of whether the "3 profiles need attention" banner should be scoped.

5. **NEW: Phase ordering -- C before D or D before C?**: Tech-designer identified that Phase D (persistence) enables instant startup badge rendering for Phase C. If startup UX is a priority, consider reordering. If trend tracking is lower priority, keep original C-then-D order.

---

## Open Questions

### Answered During Research

| Question | Answer | Source |
| --- | --- | --- |
| **Startup time budget?** | ~400ms desktop, up to 2s on Steam Deck SD card. Acceptable async. | Tech designer |
| **DLL injection paths?** | Phase B, not MVP. | Business analyzer, UX researcher |
| **Community tap integration?** | Must-have for Phase A. | UX researcher, business analyzer |
| **NEW: Can failure trends be queried?** | Yes. `MetadataStore::query_failure_trends(days)` already implemented and tested. | Codebase analysis (metadata/mod.rs:437-483) |
| **NEW: Can last success be queried?** | Yes. `MetadataStore::query_last_success_per_profile()` already implemented. | Codebase analysis (metadata/mod.rs:401-435) |
| **NEW: Is launcher drift queryable?** | Yes. `launchers.drift_state` column populated by `launcher_sync`. | Codebase analysis (metadata/launcher_sync.rs) |
| **NEW: Migration version?** | Schema is at v5 (5 migrations). Health persistence would be v6. | Codebase analysis (metadata/migrations.rs) |
| **NEW: Should health use a new table or project from existing?** | New table. Path health and launch history are fundamentally different data. | Tech-designer trade-off analysis |

### Still Open

1. **How many profiles do typical users have?** Performance model assumes 50. Real-world profiling needed. *Unchanged.*

2. **Should health status be visible in the CLI?** Trivial to wire since logic is in `crosshook-core`. *Unchanged.*

3. **NEW: What failure trend threshold triggers a warning?** If a profile has >50% failure rate in 7 days, should the health dashboard flag it? Needs business rule for threshold definition.

4. **NEW: Should health snapshots be retained indefinitely or have a TTL?** Phase D persistence needs a retention policy. Recommendation: keep 90 days of daily snapshots, prune older entries.

---

## Downstream Feature Updates (NEW Section)

### #49 Diagnostic Bundle Export

**Original**: Health report becomes a JSON section in the bundle.
**Updated**: The diagnostic bundle can now include three sections:
1. **Filesystem health** (Phase A): `ProfileHealthResult` per profile
2. **Launch history summary** (Phase B): failure trends, last success per profile from MetadataStore
3. **Launcher drift report** (Phase B): per-launcher drift state from MetadataStore

All paths must be sanitized (W-3, W-5). The bundle schema should be designed to accommodate all three sections even if only filesystem health is available (Phase A).

### #48 Proton Migration Tool

**Original**: Health check identifies "Proton path missing" as migration trigger.
**Updated**: With launch history, the migration tool can be smarter:
- `query_failure_trends(7)` can identify profiles that started failing *after* a Proton update
- `profile_name_history` tracks when profiles were last modified, helping distinguish "user changed something" from "Proton auto-updated"
- The migration tool can prioritize profiles by failure frequency, not just by missing-path detection

### #64 Stale Launcher Detection

**Original**: `LauncherInfo::is_stale` populated during health check by comparing launcher paths vs current profile.
**Updated**: `launcher_sync` already tracks `drift_state` with values `aligned`, `missing`, `moved`, `stale`, `unknown`. The health dashboard's Phase B enrichment surfaces this directly. **#64 may be partially or fully addressed by Phase B of the health dashboard**, reducing it to a UI display task rather than a full detection implementation. Recommend reviewing #64 scope after Phase B ships.

---

## Teammate Research Artifacts

| Teammate | Document | Key Contributions | v2 Additions |
| --- | --- | --- | --- |
| API researcher | `research-external.md` | Evaluated 5 batch validation strategies. Critical finding: `steam_client_install_path` injection. | Confirmed MetadataStore query APIs are ready for use. |
| Business analyzer | `research-business.md` | Domain complexity (LOW), ~80% existing infrastructure. Hardened business rules. | Reviewed Business Rule #8 revision for metadata persistence. |
| Tech designer | `research-technical.md` | Architectural decisions, performance model, 4-state health model. | 4 trade-off analyses: persist vs. on-demand, new table vs. projection, module location, MetadataStore integration layer. Key insight: cached snapshots enable instant startup badge rendering. |
| UX researcher | `research-ux.md` | Competitive analysis, locked Phase 1 list, inline badges recommendation. | Progressive disclosure for metadata enrichment data. |
| Security researcher | `research-security.md` | 3 warnings, 5 advisories. No critical issues. | W-4 (persisted paths), W-5 (launcher paths), A-6 (soft-deleted profiles in collection queries). |
| Practices researcher | `research-practices.md` | Reuse inventory, KISS assessment, module placement. | Confirmed Phase A stays metadata-free. Enrichment is additive composition. |
