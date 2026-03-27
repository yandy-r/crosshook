# Profile Health Dashboard: Recommendations & Risk Assessment

**Feature**: Profile health dashboard with staleness detection (GitHub #38)
**Phase**: 2 (Diagnostics & Health), Order #4
**Dependencies**: #39 (actionable validation errors) -- DONE
**Downstream**: #49 (diagnostic bundle export), #48 (Proton migration tool)
**Last updated**: 2026-03-27
**Research team**: Synthesized from API research, business analysis, technical design, UX research, security evaluation, and engineering practices review.

---

## Executive Summary

The profile health dashboard can be built almost entirely on existing infrastructure. Domain complexity is **LOW** -- approximately 80% of the logic already exists in `validate_all()`, `ValidationError::help()`, `ProfileStore::list()`, and `ProfileStore::load()`. The `CompatibilityBadge` component provides a proven badge pattern for tiered health status. The primary new work is: (1) a Rust-side `GameProfile -> LaunchRequest` conversion function, (2) a new `ProfileHealthInfo` struct in `profile/health.rs`, (3) a `profile_health_check_all` Tauri command, and (4) a frontend health badge component. The feature is medium complexity with low risk if scoped correctly -- the biggest danger is over-engineering the first iteration with file watchers, caching, or auto-repair.

---

## Implementation Recommendations

### Technical Approach

#### Key Architectural Decisions (Synthesized from all 6 researchers)

| Decision                           | Recommendation                                                | Rationale                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **New module vs. extend existing** | New `profile/health.rs` file (not top-level `health/` module) | Health checks filesystem paths at rest; launch validation checks runtime constraints. One new file is sufficient -- `profile/mod.rs` adds `pub mod health;`. (practices-researcher confirmed, tech-designer agreed)                                                                                                                                                                                                                        |
| **Sync vs. async batch**           | Start synchronous via `spawn_blocking`                        | 50 profiles x 8 paths x ~1ms per `Path::exists()` = ~400ms worst case. Acceptable for synchronous `invoke`. Do NOT use `tokio::fs` (each call is `spawn_blocking` internally per tokio docs) or `rayon` (wrong fit for I/O-bound work). (api-researcher Option A, tech-designer confirmed)                                                                                                                                                 |
| **Health status granularity**      | Tri-state display: Healthy / Stale / Broken                   | Aligns with issue #38 spec and `LauncherInfo::is_stale` precedent. Business rule (hardened): configured path + `Path::exists() == false` = always **Stale** (covers Proton update, game uninstall, SD card unmount). **Broken** = structural invalidity (parse errors, unsupported method). Security advisory: internally distinguish `Missing` (ENOENT) from `Inaccessible` (EACCES) for remediation text selection, but show same badge. |
| **Caching**                        | No caching (always re-check)                                  | Filesystem state changes at any time. Check is fast enough. Display "last checked at" timestamp. (tech-designer, security A-3 TOCTOU)                                                                                                                                                                                                                                                                                                      |
| **Health issue type**              | Reuse `LaunchValidationIssue` (not new type)                  | Practices-researcher pushed back on new `HealthIssue` type: existing `help` text is already remediation guidance, `ValidationError` variant names are the implicit field discriminant. Avoids parallel type hierarchy. If Phase 2 needs `field` discriminant for action buttons, extend then.                                                                                                                                              |
| **Startup validation**             | Always-on async, non-blocking, passive                        | Spawn async task after UI renders. Emit `profile-health-batch-complete` Tauri event. No modal, no blocking. Badges appear when user opens profile list. Summary banner only for broken profiles. Must NOT be added to synchronous `startup.rs` path. (business-analyzer, ux-researcher, tech-designer all aligned)                                                                                                                         |
| **Batch concurrency**              | Sequential or bounded (4 concurrent)                          | Avoid I/O pressure on slow storage (SD card). Sequential is simpler and fast enough. (security A-4)                                                                                                                                                                                                                                                                                                                                        |

**Critical correctness requirement** (from api-researcher): The `steam_applaunch` validation requires `steam_client_install_path` from `AppSettings` (via `derive_steam_client_install_path()` in `commands/profile.rs`), NOT from the profile. If the health check command doesn't inject this from settings, all Steam profiles will show false "Broken" status. The `derive_steam_client_install_path()` helper must move from `src-tauri/src/commands/profile.rs` into `crosshook-core` so the health module can use it.

#### Core Architecture: Profile-to-Validation Bridge

The main gap is that `validate_all()` operates on `LaunchRequest`, not `GameProfile`. The conversion currently happens implicitly in the frontend (`useProfile.ts::normalizeProfileForEdit`). The health dashboard needs this in Rust.

**Recommended approach**: Add a `GameProfile::to_launch_request()` method to `crosshook-core/src/profile/models.rs` that constructs a `LaunchRequest` from profile fields. Use `resolve_launch_method(profile)` first, then check only method-relevant paths (mirroring `validate_all()`'s dispatch logic). This is the single most important piece of new code.

```
GameProfile -> resolve_launch_method() -> to_launch_request() -> validate_all() -> Vec<LaunchValidationIssue>
                                                                                         |
                                                                                         v
                                                                        derive ProfileHealthStatus from severity
```

Then build a `ProfileHealthInfo` struct in a new `crosshook-core/src/profile/health.rs` file:

- `name: String` -- profile identifier
- `status: ProfileHealthStatus` -- enum: `Healthy`, `Stale`, `Broken`
- `issues: Vec<HealthIssue>` -- new struct with field, path, message, remediation, severity
- `checked_at: String` -- ISO 8601 timestamp

The `ProfileHealthStatus` is derived from the highest-severity issue: any fatal -> Broken, any warning -> Stale, none -> Healthy.

**Path utility promotion**: The private functions `require_directory()`, `require_executable_file()`, and `is_executable_file()` in `request.rs` should be promoted to `pub(crate)` -- they already have 3 call sites and the health module needs them as core path-stat primitives.

**`derive_steam_client_install_path()` relocation**: This function currently lives in `src-tauri/src/commands/profile.rs:13` and derives the Steam client install path from the compatdata path. It must move to `crosshook-core` so the health check can construct complete `LaunchRequest`s for Steam profiles without depending on the Tauri layer. This is the only substantive code migration needed beyond the new health module.

#### Batch Validation Command

Expose via Tauri as `profile_health_check_all`:

1. Call `ProfileStore::list()` to get all profile names
2. For each name, call `ProfileStore::load()` (catch `TomlDe` errors per-profile, report as "Broken (parse error)")
3. Call `profile.to_launch_request()` then `validate_all()`
4. Collect results into `Vec<ProfileHealthInfo>`
5. Return to frontend

Also expose `profile_health_check(name)` for single-profile checks (useful for re-checking after fixes).

**Important**: Invalidate/re-check when any profile is saved via `profile_save` to avoid stale-after-fix confusion.

#### Frontend Integration

Reuse `CompatibilityBadge` CSS class pattern (`crosshook-status-chip crosshook-compatibility-badge--{rating}`) for health status chips. Add a `ProfileHealthSummary` component that shows:

- Aggregate counts: "8 healthy, 2 need attention, 1 cannot launch"
- Per-profile expandable detail using `CollapsibleSection`
- Click-to-navigate to the affected profile in the editor

### Phasing: What to Build First

**Phase A (MVP -- 1-2 days)**:

1. `GameProfile::to_launch_request()` in `crosshook-core`
2. `profile_health_check_all` Tauri command
3. Simple frontend list showing health results with badge per profile

**Phase B (Polish -- 1-2 days)**:

1. On-demand "Check Health" button in the profile sidebar
2. Per-issue remediation hints (reuse `ValidationError::help()` text)
3. Filter/sort by health status

**Phase C (Startup integration -- 0.5 days)**:

1. Optional startup health check (behind settings toggle)
2. Non-blocking: emit `profile-health` Tauri event, don't block UI init

### Quick Wins

| Win                                                  | Effort  | Rationale                                             |
| ---------------------------------------------------- | ------- | ----------------------------------------------------- |
| Reuse `CompatibilityBadge` for health chips          | Minutes | Same CSS class pattern, same tiered colors            |
| Reuse `CollapsibleSection` for detail panels         | Minutes | Already used in LaunchPanel and CompatibilityViewer   |
| Reuse `ValidationError::help()` for remediation text | Zero    | Already written for all 20+ validation error variants |
| Reuse `sanitize_display_path()` for path display     | Minutes | Already strips `$HOME` to `~`                         |

### Leveraging Existing validate() Infrastructure

The `validate_all()` function (`request.rs:442`) is purpose-built for this:

- Collects _all_ issues instead of fail-fast (unlike `validate()` which returns first error)
- Returns `Vec<LaunchValidationIssue>` with structured `message`, `help`, `severity`
- Method-specific collectors: `collect_steam_issues()`, `collect_proton_issues()`, `collect_native_issues()`
- Already checks path existence, directory validity, executable permissions, optimization conflicts

What `validate_all()` does NOT check (and health dashboard should add):

- Trainer paths when `launch_game_only = true` (currently skipped)
- Icon path existence (`launcher.icon_path`)
- DLL injection paths (`InjectionSection.dll_paths`) -- silently ignored by `validate_all()` today; a profile with a missing DLL will show as Healthy but fail at launch
- Whether Proton version is still installed (higher-level check)
- Profile TOML parse errors (would fail at `ProfileStore::load()`)

### Edge Cases Requiring Special Handling

1. **Empty/unconfigured profiles**: A brand-new profile with all-default empty values will fire `Required` errors from `validate_all()` and be classified as "Broken." This is alarming for users who haven't finished setup. **Recommendation**: Introduce a distinct "Unconfigured" state for profiles where `game.executable_path` is empty, filtering them out of health results or showing them separately.

2. **Community-imported profiles** (must-have per UX + business): Profiles imported from community taps reference paths specific to another user's system. They appear immediately with many missing paths. **Recommendation**: When a profile has `community_tap_url` metadata and multiple missing-path issues, show: "This profile was imported -- paths may need to be updated for your system. Use Auto-Populate to configure." This is a must-have to avoid users blaming CrossHook for a configuration mismatch.

3. **Removable media (Steam Deck SD card)**: Steam library paths on unmounted SD cards make profiles appear broken. **Hardened business rule**: configured path + `Path::exists() == false` = always **Stale**, not **Broken**, regardless of cause. The system cannot distinguish unmounted media from deleted files; Stale is the correct conservative classification.

4. **Proton auto-updates**: Steam updates Proton from e.g. 9.0-1 to 9.0-2 silently. The old path no longer exists. This is a normal lifecycle event, not a bug. **Recommendation**: Detect the pattern (path matches `*/Proton */proton` and parent no longer exists) and show "Proton updated -- re-run Auto-Populate" rather than generic "path missing."

5. **Remediation text context**: `ValidationError::help()` was written for launch-time context ("before launching, browse to..."). Some messages may read slightly awkward in a health dashboard context. This is low risk since the text is still correct and actionable -- reuse verbatim for Phase 1. (business-analyzer confirmed low priority)

6. **Missing vs. inaccessible paths** (security advisory A-1): Internally distinguish `ENOENT` (file gone) from `EACCES` (file exists but permissions wrong). They look identical visually (both show Stale badge) but require different remediation copy: "file not found, re-browse" vs. "file exists but permission denied, check file ownership." Same implementation cost, better UX.

7. **`steam_client_install_path` injection** (api-researcher critical finding): Steam profiles require `steam_client_install_path` derived from settings/compatdata path, NOT stored in the profile itself. The health check must call `derive_steam_client_install_path()` when building the `LaunchRequest`. Without this, all Steam profiles produce false "Broken" status.

---

## Improvement Ideas

### Related Features This Enables

| Feature                  | Issue | How Health Dashboard Helps                                                                                   |
| ------------------------ | ----- | ------------------------------------------------------------------------------------------------------------ |
| Diagnostic bundle export | #49   | Health report becomes a section in the bundle -- JSON-serializable `Vec<ProfileHealthReport>`                |
| Proton migration tool    | #48   | Health check identifies "Proton path missing" as the trigger for migration workflow                          |
| Stale launcher detection | #64   | `LauncherInfo::is_stale` can be populated during health check by comparing launcher paths vs current profile |
| Import wizard            | #45   | Health check validates imported profiles immediately, surfacing issues before first launch                   |

### Future Enhancements (Not for Phase 1)

1. **Auto-repair for common issues**: When Proton path is missing but a newer version exists in the same location, offer one-click update. Depends on Steam discovery (`steam/` module) being able to suggest alternatives.

2. **File watching (inotify)**: Monitor profile directories for external changes and re-run health checks. Adds complexity for minimal benefit since profiles change infrequently. Consider only if users report confusion about stale health status.

3. **Health history / trend**: Track health status over time to detect recurring breakage patterns. Over-engineering for Phase 1 but could be useful for power users with many profiles.

4. **Batch repair**: "Fix all stale Proton paths" button that runs Auto-Populate across affected profiles. Natural extension of #48 (Proton migration tool).

5. **Profile quality score**: Beyond binary health, rate profiles on completeness (has icon? has display name? has game name?). Low priority but useful for community profile export quality.

---

## Risk Assessment

### Technical Risks

| Risk                                                                                 | Likelihood | Impact                                         | Mitigation                                                                                                                                                                                                                       |
| ------------------------------------------------------------------------------------ | ---------- | ---------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `GameProfile -> LaunchRequest` conversion logic diverges from frontend normalization | Medium     | High -- false health results                   | Write comprehensive tests comparing Rust conversion output with known-good frontend normalization for sample profiles. Use `tempfile::tempdir()` + `ProfileStore::with_base_path()` pattern from existing `toml_store.rs` tests. |
| `steam_client_install_path` not injected into health check `LaunchRequest`           | High       | High -- all Steam profiles show false "Broken" | Relocate `derive_steam_client_install_path()` to `crosshook-core`; health check command must accept or derive this from `AppSettings`. **Blocking for MVP.**                                                                     |
| Batch validation I/O blocks Tauri main thread                                        | Low        | Medium -- UI freezes                           | Start synchronous (50 profiles x 8 paths x ~1ms = ~400ms is acceptable). Profile if real-world numbers differ. Use `tokio::task::spawn_blocking` only if needed.                                                                 |
| Profile TOML parse errors crash batch validation                                     | Medium     | Medium -- one bad profile breaks all results   | Catch `ProfileStoreError::TomlDe` per profile, report as "Broken (parse error)" rather than propagating                                                                                                                          |
| Health check reports false "broken" for removable media (SD card)                    | Medium     | Low -- user confusion                          | Hardened business rule: configured path + missing = always **Stale**, never **Broken**. Covers SD card unmount, Proton updates, game uninstalls.                                                                                 |
| `validate_all()` internal changes break health dashboard                             | Low        | Medium                                         | Health module depends on `validate_all()` public API, which is stable; add integration tests                                                                                                                                     |
| Empty profiles classified as "Broken" alarm new users                                | Medium     | Medium -- bad first impression                 | Introduce "Unconfigured" state for profiles with empty `game.executable_path`; filter from health results or show separately                                                                                                     |
| Community-imported profiles appear immediately broken                                | Medium     | Low -- expected but jarring                    | Show targeted "use Auto-Populate to configure" message instead of generic "Broken"                                                                                                                                               |

### Security Risks (from security researcher)

| ID  | Severity | Risk                                                                                                                            | Mitigation                                                                                                                                      |
| --- | -------- | ------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| W-1 | Warning  | **CSP disabled** (`tauri.conf.json:23`, `"csp": null`) -- not caused by this feature but any new IPC surface increases exposure | Enable CSP before shipping health dashboard. Pre-ship security item.                                                                            |
| W-2 | Warning  | **Raw paths in IPC responses** -- health check returns filesystem paths to frontend                                             | Apply `sanitize_display_path()` to all path fields in `ProfileHealthInfo` before returning over IPC.                                            |
| W-3 | Warning  | **Diagnostic bundle path leak** (#49 downstream) -- health reports exported in bundle expose filesystem layout                  | Sanitize all paths in exported health JSON. Higher-risk surface than in-app display. Address when #49 is implemented.                           |
| A-1 | Advisory | **ENOENT vs EACCES conflation** -- `Path::exists()` returns `false` for both missing and permission-denied                      | Use `std::fs::metadata()` to distinguish; select appropriate remediation text ("not found" vs "permission denied"). Same badge, different copy. |
| A-3 | Advisory | **TOCTOU** -- path could disappear between health check and launch                                                              | Accepted risk. Health check is informational, not a gate. Launch still validates independently.                                                 |
| A-4 | Advisory | **Batch I/O concurrency on slow storage** -- parallel path checks on SD card could thrash I/O                                   | Sequential validation is simpler and fast enough. If parallelized, bound to 4 concurrent.                                                       |

### Performance Risks

- **Profile count scaling**: `ProfileStore::list()` + `load()` for each profile is O(n) filesystem reads. For typical users (5-20 profiles), this is <100ms. For edge cases (100+ profiles), could reach 1-2 seconds.
- **Concrete performance model** (from tech-designer): 50 profiles x 8 paths x ~1ms per `Path::exists()` = ~400ms. File existence checks are I/O-bound, not CPU-bound. This is within acceptable UX budget for on-demand validation.
- **Startup time impact**: If health check runs at startup, it adds filesystem I/O proportional to profile count. Mitigation: run async ~500ms after startup via Tauri event, don't block render.
- **Steam Deck storage**: ARM64 Zen 2 APU with eMMC/NVMe/SD card storage. Budget 2-5x slower filesystem operations vs desktop Linux NVMe. 400ms desktop -> up to 2s on SD card.
- **Recommendation**: Start with on-demand only (button click). Add optional startup check in Phase C after measuring real-world performance.
- **No new dependencies**: Everything covered by `std::fs`, `std::path`, `std::os::unix::fs::PermissionsExt`, and `tempfile` (already a dev-dep). Do not add `walkdir`, `notify`, or `rayon` for v1.

### UX Risks

- **Warning fatigue**: Users with many Steam games may have profiles where Proton was auto-updated by Steam. Showing 15 "Proton path changed" warnings on startup would be overwhelming.
  - Mitigation: Show aggregate summary ("3 profiles need attention") with expandable detail, not a wall of alerts.
- **Stale-after-fix confusion**: If user fixes a path in ProfileEditor and returns to health view without re-checking, stale badges persist.
  - Mitigation: Invalidate health results when any profile is saved (hook into `profile_save` command).
- **Unclear remediation**: Some issues require complex manual steps (reinstall Proton, reverify game files). The existing `ValidationError::help()` text covers most cases well but was written for launch-time context.
- **Status label clarity**: "Stale" is ambiguous -- does it mean "old" or "broken"? Use concrete labels: "Healthy", "Needs Attention", "Cannot Launch" instead of abstract terms.

---

## Alternative Approaches

### UI Placement (Resolved: Option B)

| Option                                   | Verdict                    | Rationale                                                                                                                                                                                                |
| ---------------------------------------- | -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **A: Dedicated Dashboard Tab**           | Deferred                   | Adds navigation complexity; may feel empty for users with few profiles. Consider only if users request batch management across many profiles.                                                            |
| **B: Inline Badges in Profile Selector** | **Selected**               | Zero navigation changes; health visible in context; leverages existing `CompatibilityBadge` pattern; natural discovery. Lowest-effort, highest-impact. UX researcher and business analyzer both aligned. |
| **C: Toast/Notification**                | Rejected as sole mechanism | Easy to dismiss and forget; poor for batch results. May complement Option B for startup summary banner only.                                                                                             |
| **D: Collapsible Sidebar Section**       | Deferred                   | Takes sidebar space; sidebar may not exist in current layout. Revisit if Option B feels cramped.                                                                                                         |

### Batch Validation Strategy (Evaluated by API researcher)

| Option | Approach                                          | Verdict      | Rationale                                                                                            |
| ------ | ------------------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------- |
| **A**  | Synchronous `validate_all()` via `spawn_blocking` | **Selected** | 50 profiles x 8 paths x ~1ms = ~400ms. Acceptable synchronous. Simplest implementation.              |
| **B**  | `notify` crate file watcher                       | Rejected     | Wrong tool for on-demand checks. Adds dependency, complexity. Profiles change infrequently.          |
| **C**  | `rayon` parallel validation                       | Rejected     | Wrong fit for I/O-bound work. `rayon` is designed for CPU-bound parallelism.                         |
| **D**  | `tokio::fs` async per-file                        | Rejected     | Each `tokio::fs` call is `spawn_blocking` internally per tokio docs. More overhead than direct sync. |
| **E**  | Background polling timer                          | Rejected     | Unnecessary complexity. Profiles rarely change without user action.                                  |

### Startup Behavior (Resolved: Always-on Async)

| Approach                                | Verdict                  | Rationale                                                                                                                                                                                                                                                 |
| --------------------------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| On-demand only (button click)           | Phase A fallback         | Simplest but users may forget to check.                                                                                                                                                                                                                   |
| Startup (blocking)                      | Rejected                 | Slows startup; bad on slow storage. Must NOT touch synchronous `startup.rs` path.                                                                                                                                                                         |
| **Startup (non-blocking event)**        | **Selected for Phase C** | Fresh data without blocking. Spawn async ~500ms after UI renders. Emit `profile-health-batch-complete` Tauri event. Passive badges, no modal. Business analyzer, UX researcher, and tech designer all aligned on always-on async as default (not opt-in). |
| Background timer (poll every N minutes) | Rejected                 | Profiles rarely change without user action.                                                                                                                                                                                                               |

---

## Task Breakdown Preview

### Pre-ship Security Items

| Task                                                                   | File(s)                             | Complexity | Blocking?                                     |
| ---------------------------------------------------------------------- | ----------------------------------- | ---------- | --------------------------------------------- |
| Enable CSP in Tauri config (W-1)                                       | `src-tauri/tauri.conf.json`         | Low        | Yes -- do before shipping any new IPC surface |
| Apply `sanitize_display_path()` to all health report path fields (W-2) | `src-tauri/src/commands/profile.rs` | Trivial    | Yes -- required for health check command      |

### Phase A: Core Health Check (MVP) -- Locked Scope

_8 must-have items from UX researcher's locked Phase 1 list. Any proposal exceeding this is over-engineering._

| #   | Task                                                                                                                     | File(s)                                                                                               | Complexity | Dependencies                                             |
| --- | ------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------- | ---------- | -------------------------------------------------------- |
| 1   | Relocate `derive_steam_client_install_path()` to `crosshook-core`                                                        | `src-tauri/src/commands/profile.rs` -> `crates/crosshook-core/src/profile/models.rs` (or `health.rs`) | Low        | None -- **blocking for all Steam profile health checks** |
| 2   | Promote `require_directory()`, `require_executable_file()`, `is_executable_file()` to `pub(crate)`                       | `crates/crosshook-core/src/launch/request.rs`                                                         | Trivial    | None                                                     |
| 3   | Add `GameProfile::to_launch_request()` conversion method                                                                 | `crates/crosshook-core/src/profile/models.rs`                                                         | Medium     | Task 1 (needs `derive_steam_client_install_path`)        |
| 4   | Create `ProfileHealthStatus` enum + `ProfileHealthInfo` struct (reuse `LaunchValidationIssue` for issues)                | New: `crates/crosshook-core/src/profile/health.rs`                                                    | Low        | Task 3                                                   |
| 5   | Implement `check_profile_health()` and `batch_check_health()` with "Unconfigured" detection and community-import context | `crates/crosshook-core/src/profile/health.rs`                                                         | Medium     | Task 4, `validate_all()`                                 |
| 6   | Write Rust unit tests (use `tempfile::tempdir()` + `ProfileStore::with_base_path()` pattern)                             | `crates/crosshook-core/src/profile/health.rs`                                                         | Medium     | Task 5                                                   |
| 7   | Add `profile_health_check_all` and `profile_health_check` Tauri commands (sanitize paths before return)                  | `src-tauri/src/commands/profile.rs`                                                                   | Low        | Task 5, pre-ship W-2                                     |
| 8   | Create `ProfileHealthBadge` component (reuse `crosshook-status-chip` CSS)                                                | New: `src/components/ProfileHealthBadge.tsx`                                                          | Low        | None (frontend only)                                     |
| 9   | Create `useProfileHealth` hook (mirrors `useLaunchState` reducer pattern)                                                | New: `src/hooks/useProfileHealth.ts`                                                                  | Medium     | Task 7                                                   |
| 10  | Add inline health badges to profile selector list                                                                        | `src/components/ProfileEditor.tsx`                                                                    | Low        | Tasks 8-9                                                |
| 11  | Add per-issue remediation hints (reuse `ValidationError::help()` text verbatim)                                          | `src/components/ProfileEditor.tsx`                                                                    | Low        | Tasks 9-10                                               |
| 12  | Invalidate/re-check health when any profile is saved (hook into `profile_save`)                                          | `src/hooks/useProfileHealth.ts`                                                                       | Low        | Task 9                                                   |

**Change count**: 1 new Rust file, 1 relocated function, 1 new Tauri command, 1 new hook, 1 new component, 3-4 modified files.
**Estimated complexity**: Medium (3-5 days)

### Phase A Explicit Deferrals (UX researcher)

These are NOT in Phase A scope. Do not add them:

- DLL injection path checking (Phase B)
- Icon path validation (Phase B)
- Filter/sort profiles by health status (Phase B)
- File watching / `notify` crate (not planned)
- Background polling timer (not planned)
- Auto-repair / batch fix (Phase D / #48)
- Health history / trend tracking (not planned)

### Phase B: Detail & Remediation UI

| Task                                                                                                       | Complexity | Dependencies |
| ---------------------------------------------------------------------------------------------------------- | ---------- | ------------ |
| Add health detail section to `ProfileEditor` with `CollapsibleSection`                                     | Low        | Phase A      |
| Add "Check Health" button to profile sidebar/toolbar                                                       | Low        | Phase A      |
| Add path-specific checks beyond `validate_all()`: DLL injection paths, icon path, Proton version installed | Medium     | Phase A      |
| Add filter/sort profiles by health status                                                                  | Low        | Phase A      |
| Distinguish ENOENT vs EACCES for remediation text (security A-1)                                           | Low        | Phase A      |

**Estimated complexity**: Low-Medium (1-2 days)

### Phase C: Startup Integration

| Task                                                                                                        | Complexity | Dependencies |
| ----------------------------------------------------------------------------------------------------------- | ---------- | ------------ |
| Implement always-on non-blocking startup health check via Tauri event (spawn async ~500ms after UI renders) | Medium     | Phase A      |
| Emit `profile-health-batch-complete` event; passive badges appear when user opens profile list              | Low        | Phase A      |
| Add startup summary banner for broken profiles only (non-blocking, non-modal)                               | Low        | Phase A      |

**Note**: Team consensus is always-on async (not opt-in toggle). No settings change needed.
**Estimated complexity**: Low (0.5-1 day)

### Phase D: Downstream Integration Points

| Task                                                                                        | Complexity | Dependencies |
| ------------------------------------------------------------------------------------------- | ---------- | ------------ |
| Export health report as JSON section in diagnostic bundle (#49) -- sanitize all paths (W-3) | Low        | Phase A, #49 |
| Use health check results to trigger Proton migration flow (#48)                             | Medium     | Phase A, #48 |
| Populate `LauncherInfo::is_stale` from health check results (#64)                           | Low        | Phase A      |

**Estimated complexity**: Medium (depends on downstream features)

---

## Key Decisions

### Resolved by Team Consensus

| #   | Decision                          | Resolution                                                                                                                          | Resolved By                                     |
| --- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| 1   | **Frontend placement**            | Inline badges (Option B) -- zero navigation changes, highest impact                                                                 | UX researcher, business analyzer                |
| 2   | **Startup behavior**              | Always-on async, non-blocking, passive -- spawn ~500ms after UI, emit event, no modal                                               | Business analyzer, UX researcher, tech designer |
| 3   | **Issue type design**             | Reuse `LaunchValidationIssue` -- avoid parallel type hierarchy. Extend in Phase B if `field` discriminant needed for action buttons | Practices researcher (pushed back on new type)  |
| 4   | **Removable media policy**        | Configured path + missing = always **Stale**, never **Broken** -- covers SD card unmount, Proton update, game uninstall             | Business analyzer (hardened rule)               |
| 5   | **Unconfigured profile handling** | Introduce "Unconfigured" state for profiles with empty `game.executable_path` -- filter out or show separately                      | Business analyzer, UX researcher                |
| 6   | **Module placement**              | `profile/health.rs` (not top-level `health/` module) -- one new file                                                                | Practices researcher                            |
| 7   | **Batch strategy**                | Synchronous `validate_all()` via `spawn_blocking` -- reject notify/rayon/tokio::fs/polling                                          | API researcher                                  |

### Still Open

1. **Health status labels**: Use "Healthy / Needs Attention / Cannot Launch" (user-friendly) or "Healthy / Stale / Broken" (developer-oriented)? Tech-designer recommends tri-state matching `LauncherInfo::is_stale` precedent. UX-facing labels TBD during implementation.

2. **Proton version staleness**: When Steam updates Proton from 9.0-1 to 9.0-2, is the old profile "stale" or "broken"? The old path no longer exists. Per hardened business rule, this is **Stale** (missing path). Consider adding pattern detection (path matches `*/Proton */proton`) for targeted "Proton updated -- re-run Auto-Populate" remediation text.

---

## Open Questions

### Answered During Research

| Question                       | Answer                                                                                   | Source                                               |
| ------------------------------ | ---------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| **Startup time budget?**       | ~400ms desktop, up to 2s on Steam Deck SD card. Acceptable for async non-blocking.       | Tech designer performance model                      |
| **DLL injection paths?**       | Phase B, not MVP. `validate_all()` doesn't check them today.                             | Business analyzer, UX researcher (explicit deferral) |
| **Community tap integration?** | Must-have for Phase A. Show "imported -- use Auto-Populate" instead of generic "Broken." | UX researcher, business analyzer                     |

### Still Open

1. **How many profiles do typical users have?** Performance model assumes 50. If most users have <10, batch validation is trivially fast (<100ms). Real-world profiling needed.

2. **Should health status be visible in the CLI (`crosshook-cli`)?** Since `check_profile_health()` and `batch_check_health()` live in `crosshook-core`, a `crosshook health` CLI command would be trivial to wire. Not needed for v1 but natural extension for Phase 5 (#43 CLI completion).

---

## Teammate Research Artifacts

| Teammate             | Document                | Key Contributions                                                                                                                                                                                                                                                                                 |
| -------------------- | ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| API researcher       | `research-api.md`       | Evaluated 5 batch validation strategies (sync recommended). **Critical finding**: `steam_client_install_path` injection required for Steam profiles. No external dependencies needed.                                                                                                             |
| Business analyzer    | `research-business.md`  | Domain complexity (LOW), ~80% existing infrastructure. Hardened business rules: missing path = always Stale. Edge cases: empty profiles, community imports, remediation context, removable media. Startup = always-on async.                                                                      |
| Tech designer        | `research-technical.md` | Architectural decisions table, sync/async analysis, performance model (50 profiles x 8 paths x 1ms = 400ms). Steam Deck constraint analysis. 4-state health model proposal.                                                                                                                       |
| UX researcher        | `research-ux.md`        | Competitive analysis (Steam, Lutris, Heroic, Grafana). Locked Phase 1 must-have list (8 items). Explicit deferrals. Inline badges (Option B) recommendation. Community-import context as must-have.                                                                                               |
| Security researcher  | `research-security.md`  | 3 warnings (CSP disabled, raw paths, diagnostic bundle leak). 5 advisories (ENOENT/EACCES, symlinks, TOCTOU, batch concurrency, IPC types). No critical issues.                                                                                                                                   |
| Practices researcher | `research-practices.md` | Reuse inventory, KISS assessment. Pushed back on new `HealthIssue` type (reuse `LaunchValidationIssue`). Module placement = `profile/health.rs`. Minimal change count: 1 new Rust file, 1 new command, 1 new hook, 1-2 new components. `derive_steam_client_install_path()` relocation confirmed. |
