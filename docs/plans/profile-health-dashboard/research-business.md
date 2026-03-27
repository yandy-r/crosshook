# Profile Health Dashboard — Business Analysis

## Executive Summary

CrossHook users silently accumulate broken profiles after game updates, Proton version changes, and trainer file moves. The profile health dashboard surfaces per-profile health status (healthy / stale / broken) in batch by validating **persisted profile paths at rest** — distinct from launch-time validation which checks runtime configuration compatibility. The feature uses `ProfileStore::list()` + `ProfileStore::load()` for batch loading, a new `validate_profile_health(profile: &GameProfile)` function in `crosshook-core/src/profile/` for path-existence checks (operating directly on `GameProfile`, NOT routing through `LaunchRequest`), and presents results using the `crosshook-status-chip` badge pattern from `CompatibilityViewer`. Health status is held in-memory only — never persisted to disk.

---

## User Stories

### Primary Users

- **Steam Deck users** managing multiple game profiles — most affected because SteamOS auto-updates Proton versions, silently breaking saved `proton_path` fields
- **Linux gamers** who install/uninstall Proton versions from Steam or move game libraries

### Stories

| ID   | As a…                                 | I want…                                                                                | So that…                                                                           |
| ---- | ------------------------------------- | -------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| US-1 | Steam Deck user                       | to see a health badge on each profile at a glance                                      | I know which profiles need attention before game night                             |
| US-2 | Linux gamer                           | CrossHook to check all profiles in the background at startup                           | I get notified of broken configs after Proton auto-updates without any extra steps |
| US-3 | User with many profiles               | to see a summary count ("3 of 12 profiles have issues") rather than a wall of warnings | I'm not overwhelmed and can triage what matters                                    |
| US-4 | User with a broken profile            | to see the specific path that failed and a fix suggestion                              | I know exactly what to do to fix it                                                |
| US-5 | User who imported a community profile | to see immediately if the profile references paths that don't exist on my system       | I can run Auto-Populate before wasting time on a launch attempt                    |
| US-6 | User who uninstalled a game           | to see the affected profiles marked stale rather than broken                           | I understand this is a normal lifecycle event, not a configuration error           |
| US-7 | Steam Deck user in portable mode      | to not see profiles as "broken" just because my SD card isn't mounted                  | I get accurate health status for the storage that is actually available            |

---

## Business Rules

### Scope Boundary: Health vs. Launch Validation

**Profile health validation** and **launch validation** are distinct concerns that must NOT be conflated:

| Concern                           | What it checks                                                                                                                         | When it runs                           |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| **Profile health** (this feature) | Do the filesystem paths stored in `GameProfile` still exist on disk with correct types and permissions?                                | Background at startup + manual trigger |
| **Launch validation** (existing)  | Is the runtime configuration compatible? (method constraints, optimization conflicts, `steam_client_install_path` derived value, etc.) | Immediately before each launch attempt |

Health checks validate paths at rest in `GameProfile`. They do **not** validate:

- Launch optimization conflicts or dependencies
- Launch method compatibility constraints
- `steam_client_install_path` — this is derived at launch time from `compatdata_path` and is a `LaunchRequest` concern, not a stored profile field
- Whether a WINE prefix has been fully initialized
- Whether Steam App IDs resolve to real games (no network calls)
- Whether Proton prefix directories are structurally valid (shallow only)

A profile can be **health-healthy** (all paths exist) and still fail launch validation (e.g., incompatible optimization combination). Health status is a prerequisite signal, not a launch guarantee.

**Architecture constraint:** The new `validate_profile_health()` function lives in `crosshook-core/src/profile/` — not `launch/`. It takes `&GameProfile` directly, never constructs a `LaunchRequest`. This is the correct module ownership.

### Core Health Classification

**Profile-level status** — three states (plus a presentation variant):

| Status           | Condition                                                                                                             | Visual (existing CSS)                            | Notification behavior                               |
| ---------------- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | --------------------------------------------------- |
| **Healthy**      | All required fields configured; all configured paths accessible with correct type                                     | Green — `crosshook-compatibility-badge--working` | No notification                                     |
| **Stale**        | Required fields configured (non-empty); one or more configured paths missing from disk                                | Amber — `crosshook-compatibility-badge--partial` | Badge only — no banner                              |
| **Broken**       | Required field empty/unconfigured, OR path exists but wrong type, OR path exists but inaccessible (permission denied) | Red — `crosshook-compatibility-badge--broken`    | Banner notification on startup                      |
| **Unconfigured** | UI presentation variant of Broken for profiles with ALL path fields empty (never set up)                              | Same red badge — softer notification tone        | Badge only (less alarming; normal for new profiles) |

**Severity precedence:** If a profile has both Stale and Broken issues, the overall status is **Broken** (more severe wins).

**Stale vs. Broken distinction:**

- _Stale_ = the path was configured; external state changed (Proton updated, game uninstalled, trainer moved, SD card not mounted). This is a **normal lifecycle event**. Profile data is not misconfigured — the world changed.
- _Broken_ = the profile data itself is incomplete (required field empty) or self-contradictory (path points to wrong filesystem type, or file exists but can't be accessed). User must edit the profile or fix permissions.

### Path Issue Classification (Field Level)

For each path field in `GameProfile`, the health check produces one of four per-field states:

| Field state       | Filesystem condition                                                           | Profile impact                                                |
| ----------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------- |
| **NotConfigured** | Field is empty string                                                          | Required field → Broken; Optional field → no issue (expected) |
| **Missing**       | Path non-empty, `Path::exists()` → false (ENOENT or unmounted media)           | → **Stale**                                                   |
| **Inaccessible**  | Path exists, but permission denied on stat/access (EACCES)                     | → **Broken** (different remediation: "fix permissions")       |
| **WrongType**     | Path exists and accessible, but is file where directory expected or vice versa | → **Broken**                                                  |
| **Healthy**       | Path exists, accessible, correct type                                          | → no issue                                                    |

**SD card / removable media rule:** `Path::exists()` returning false is always classified **Missing → Stale**, regardless of whether the cause is a deleted file or unmounted removable storage. The system cannot distinguish these cases. Stale is the accurate conservative classification.

**Permission denied (Inaccessible) is distinct from Missing:** A file that exists but has `chmod 000` should be reported as "inaccessible — check file permissions" rather than "not found — re-browse to the file." These require different remediation and must not be conflated.

### Required vs. Optional Field Rules (Method-Aware)

Health checks are **method-aware** — they only validate fields required by the profile's resolved launch method. Empty optional fields are `NotConfigured` and produce no issue:

| Profile field              | Required for method                                       | Optional for method         | Always optional |
| -------------------------- | --------------------------------------------------------- | --------------------------- | --------------- |
| `game.executable_path`     | `proton_run`, `native`; conditional for `steam_applaunch` | —                           | —               |
| `trainer.path` (host path) | Any (when trainer workflow configured)                    | Any (when game-only)        | —               |
| `steam.proton_path`        | `steam_applaunch`                                         | `proton_run`, `native`      | —               |
| `steam.compatdata_path`    | `steam_applaunch`                                         | `proton_run`, `native`      | —               |
| `steam.app_id`             | `steam_applaunch`                                         | `proton_run`, `native`      | —               |
| `runtime.proton_path`      | `proton_run`                                              | `steam_applaunch`, `native` | —               |
| `runtime.prefix_path`      | `proton_run`                                              | `steam_applaunch`, `native` | —               |
| `injection.dll_paths[]`    | —                                                         | —                           | Always optional |
| `steam.launcher.icon_path` | —                                                         | —                           | Always optional |

**Note on `steam.app_id`:** Not a filesystem path — `NotConfigured` (empty) maps directly to Broken (can't launch steam_applaunch without it). No Missing/Inaccessible states apply.

**DLL injection paths:** Not validated in Phase 2. A missing DLL will show Healthy but fail at launch — this is a pre-existing gap. Recommend adding in a follow-on phase.

### Remediation Text

All remediation text is reused verbatim from `ValidationError::help()` (`request.rs:297–426`). No new copy is required for existing error types. The `Inaccessible` state requires new remediation text (not in existing error variants):

- **Inaccessible (file, needs execute)**: "Check file permissions — CrossHook cannot execute this file. Try: `chmod +x <path>`"
- **Inaccessible (directory, needs read)**: "Check directory permissions — CrossHook cannot read this directory."

**Context note:** Existing `ValidationError::help()` text was written for launch-time context ("before launching..."). In the health dashboard context this reads slightly awkwardly but remains correct and actionable. Low priority to rewrite for Phase 2.

**Community-imported profiles:** Will immediately appear Stale (all configured paths reference another user's system). A supplemental context hint in the detail panel is recommended: "This profile was imported from a community tap. Use Auto-Populate to configure paths for your system." This is a UI concern, not a new validation type.

### Notification Rules

**Notification is startup-only and per-session:**

- Results are shown when background scan completes after startup
- Dismissing a notification persists until next app launch (re-shown if issues still exist on next launch)
- No persistent "unread" state — notification is advisory, not requiring action

**Notification severity tiers:**

- **Broken profiles** → always show a startup banner (e.g., "2 profiles have broken paths — click to review")
- **Stale profiles** → badge only; no startup banner (stale is a normal lifecycle state)
- **Unconfigured profiles** → badge only (new profiles are expected to be unconfigured)

**No time-based staleness threshold:** Staleness is binary — a path either exists or it doesn't. Implementing a "stale for N days" threshold would require persisting validation timestamps, which conflicts with the no-persistence constraint from security. Notification suppression for stale is handled by not using banners (badge-only), not by time filtering.

### Auto-Repair Policy

**No silent auto-repair.** The health dashboard is a **read-only diagnostic** feature. It classifies and surfaces issues; it does not modify profile data automatically.

Remediation actions available to users:

1. Navigate to `ProfileEditor` via "Open Profile" CTA — edit paths manually
2. Use existing `AutoPopulate` component to re-detect paths — user-initiated, user-confirmed
3. Delete the profile via existing profile delete flow

No auto-repair actions are performed by the health check or the health dashboard UI, even for issues where CrossHook could theoretically auto-fix (e.g., re-detecting Proton). Any such action must be user-initiated.

### Validation Depth

**Phase 2: Shallow validation only.** For each path field:

1. Check `path.is_empty()` → NotConfigured
2. Check `Path::new(path).exists()` → if false → Missing
3. Check `Path::new(path).metadata()` permissions → if EACCES → Inaccessible
4. Check `Path::new(path).is_file()` or `is_dir()` → if wrong type → WrongType
5. Otherwise → Healthy

**Out of scope for Phase 2:**

- Proton prefix structural validity (is the WINE prefix properly initialized?)
- Steam AppID resolution (does this app ID resolve to a real game on Steam?)
- Executable file format validation (is the .exe a valid PE binary?)
- File content hashing or integrity verification
- Network calls of any kind

### Launch Blocking Policy

Health dashboard does **not** add new launch-blocking logic. The existing `validate_launch` Tauri command already hard-blocks on `ValidationSeverity::Fatal` issues immediately before launch. Health status is informational — a stale or broken badge does not prevent the user from attempting to launch. Launch will fail at the existing validation gate if the profile is genuinely broken at launch time.

### Batch Validation Rules

- All saved profiles are validated regardless of launch method
- Validation is **non-blocking**: must not delay app UI render on startup
- Startup health scan is a background async task spawned after UI is ready — NOT in the synchronous `startup.rs` / `resolve_auto_load_profile_name` path
- Results are held **in app frontend state (in-memory) only** — never written to disk
- Results are invalidated when any profile is saved, renamed, or deleted; user must manually trigger re-check after editing
- Manual re-check replaces all in-memory results
- A single profile TOML failing to load (parse/I/O error) is classified **Broken** with a load-error message — must not abort the batch scan

---

## Workflows

### Workflow 1: Startup Health Check (Background, Non-Blocking)

1. App starts; Tauri UI renders with profile list (no health badges yet — health panel shows "Not checked yet")
2. Immediately after UI is ready, a background async task begins
3. `ProfileStore::list()` enumerates all profile names
4. For each profile:
   a. `ProfileStore::load()` → if error → classify as Broken (load-error issue)
   b. Call `validate_profile_health(&profile)` → collect `Vec<ProfileHealthIssue>`
   c. Classify profile status: Healthy / Stale / Broken / Unconfigured
5. Background task completes → emits Tauri event (e.g., `"profile-health-results"`) with `Vec<ProfileHealthResult>`
6. Frontend receives event, updates health badges, displays summary count
7. If any profiles are Broken → show startup banner notification (non-modal, dismissable)
8. If zero profiles → no-op, health panel hidden or shows "No profiles"

### Workflow 2: Manual Health Check ("Re-check All")

1. User clicks "Re-check All" in the health panel
2. Panel shows loading state ("Checking profiles...")
3. Frontend invokes `batch_validate_profiles` Tauri command
4. Command runs same logic as startup scan, returns `Vec<ProfileHealthResult>` synchronously (or via callback)
5. Frontend replaces all cached results; badges and summary count update
6. If issues persist → banner re-shown (or summary refreshed)

### Workflow 3: Viewing Remediation Detail

1. User clicks a broken/stale profile's badge or expand arrow
2. `CollapsibleSection` (existing component) expands showing per-issue list
3. Issues sorted: Broken issues first, then Stale
4. Each issue shows: human-readable field label, issue message, remediation help text
5. "Open Profile" CTA navigates to `ProfileEditor` for that profile
6. No in-place repair; editor navigation is the remediation action

### Workflow 4: Auto-Revalidate on Save

1. User edits a profile in `ProfileEditor` and saves (`invoke('save_profile')` resolves)
2. Frontend immediately calls single-profile health check: `invoke('validate_profile', { name })`
3. Result updates only that profile's badge in-place — no full batch re-scan
4. If user navigates away before the recheck completes, badge holds its last-known state; result updates silently when it arrives (no spinner revert)
5. Full "Re-check All" remains available for post-external-change scenarios (game reinstalled, new Proton version)

### Workflow 5: Profile Load Failure

1. `ProfileStore::load()` returns error
2. Profile classified **Broken**: single load-error issue: "Profile file could not be read"
3. Remediation: "Try deleting and re-creating this profile."
4. Must not abort the batch scan — error is caught per-profile

---

## Domain Model

### Key Entities

**`GameProfile`** (existing — `crates/crosshook-core/src/profile/models.rs`)

- Source of truth for health checks — validated directly, no conversion to `LaunchRequest`

**`ProfileHealthStatus`** (new)

```
enum ProfileHealthStatus { Healthy, Stale, Broken }
```

(`Unconfigured` is a UI presentation variant of `Broken`, not a separate enum variant)

**`ProfileHealthIssue`** (new)

```
struct ProfileHealthIssue {
    field: String,         // human-readable field label ("Game Executable", "Proton Path")
    message: String,       // what's wrong
    help: String,          // remediation suggestion
    kind: HealthIssueKind, // NotConfigured | Missing | Inaccessible | WrongType
}

enum HealthIssueKind { NotConfigured, Missing, Inaccessible, WrongType }
```

`kind` drives the profile-level status classification:

- `Missing` → contributes to Stale
- `NotConfigured` / `Inaccessible` / `WrongType` → contributes to Broken

**`ProfileHealthResult`** (new)

```
struct ProfileHealthResult {
    name: String,
    status: ProfileHealthStatus,
    issues: Vec<ProfileHealthIssue>,
}
```

Returned by `batch_validate_profiles` Tauri command. Serde-serializable.

### State Transitions

```
Initial (UI rendered, scan not started)
    │ background scan starts
    ▼
Scanning...
    │ scan completes → emits "profile-health-results"
    ▼
Healthy | Stale | Broken (per profile, in frontend state)
    │
    ├── profile saved/renamed/deleted → result invalidated → badge cleared
    │
    ├── "Re-check All" clicked → Scanning... (full re-check)
    │
    └── notification dismissed → dismissed until next app launch
         └── next app launch → Scanning... (repeats)
```

---

## Existing Codebase Integration

### Directly Reusable

| Asset                         | Location                           | How it's used                                                                           |
| ----------------------------- | ---------------------------------- | --------------------------------------------------------------------------------------- |
| `ValidationError::help()`     | `request.rs:297`                   | Remediation text for all path errors — no new copy needed                               |
| `ProfileStore::list()`        | `toml_store.rs:136`                | Enumerates all profiles for batch check                                                 |
| `ProfileStore::load()`        | `toml_store.rs:100`                | Loads each profile                                                                      |
| `CompatibilityBadge` CSS      | `CompatibilityViewer.tsx:76`       | `crosshook-status-chip crosshook-compatibility-badge--{rating}` maps to health statuses |
| `CollapsibleSection`          | `components/ui/CollapsibleSection` | Used for expandable issue detail panels                                                 |
| `is_stale` concept            | `launcher_store.rs:42`             | Analogous pattern: path-existence check on stored data                                  |
| `validate_launch` IPC pattern | `commands/launch.rs:31`            | Tauri command pattern to follow                                                         |

### What NOT to Reuse

- **`validate_all(request: &LaunchRequest)`**: Includes launch-configuration checks (optimization conflicts, `steam_client_install_path`, method compatibility) that are out of scope for health checks. Health validation must NOT route through `LaunchRequest`.
- **`validate_name()` / `validate_path_traversal()`**: Profile names are already safe when loaded via `ProfileStore::list()` — no additional name validation needed in health checks.

### Gaps Requiring New Code

1. **`validate_profile_health(profile: &GameProfile, method: &str) -> Vec<ProfileHealthIssue>`** — New function in `crosshook-core/src/profile/health.rs`. Path-existence + type + permission checks directly on `GameProfile` fields, method-aware. Reuses `ValidationError::help()` for remediation text.

2. **`batch_validate_profiles` Tauri command** — In `src-tauri/src/commands/`, manages `State<ProfileStore>`, iterates profiles, calls `validate_profile_health()`, classifies status, returns `Vec<ProfileHealthResult>`.

3. **`validate_profile(name: String)` Tauri command** — Single-profile variant of the above. Called automatically after `save_profile` resolves to update that profile's badge in-place without a full batch re-scan. Returns `ProfileHealthResult` for one profile.

4. **`ProfileHealthStatus`, `ProfileHealthResult`, `ProfileHealthIssue`, `HealthIssueKind` types** — New Serde types in `crosshook-core/src/profile/health.rs`.

5. **Startup async task** — Non-blocking background task spawned from `lib.rs` Tauri setup, emits `"profile-health-results"` Tauri event on completion.

6. **Frontend health panel** — Summary count + per-profile badge list + `CollapsibleSection` detail panels. New component or extension of existing profile list.

---

## Security Constraints

- Health status results are **never persisted to disk** — in-memory only, re-computed on each app start and manual trigger
- Health check is **strictly read-only** — checks existence/metadata only, never reads binary content of game/trainer executables
- Community-tap profiles may reference any absolute paths, including sensitive system paths. The health check will confirm whether those paths exist on disk. This is acceptable within the local single-user threat model — the health check adds no new capability beyond what the user already configured
- Empty-string paths are treated as `NotConfigured` (opt-out/optional), NOT as `Missing` — this prevents false-positive "broken" classification for optional fields

---

## Success Criteria (from GitHub Issue #38)

| Criterion                                                         | Implementation path                                                                       |
| ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| All saved profiles can be validated in batch                      | `ProfileStore::list()` + `validate_profile_health()` loop                                 |
| Each profile shows health status indicator (healthy/stale/broken) | `ProfileHealthStatus` enum + `crosshook-compatibility-badge--{rating}` CSS                |
| Broken paths identified with specific remediation suggestions     | `ProfileHealthIssue.help` — reused from `ValidationError::help()` + new Inaccessible text |
| Health check runs on app startup                                  | Non-blocking async task post-UI-render, emits `"profile-health-results"`                  |
| Health check can be triggered manually                            | `batch_validate_profiles` Tauri command + "Re-check All" button                           |

---

## Open Questions

1. **`Unconfigured` profiles notification**: Brand-new all-empty profiles show as Broken (Required errors). Should the startup banner explicitly exclude profiles with ALL-empty paths (unconfigured) since this is expected for new users? Recommend: yes — detect "all NotConfigured" as `Unconfigured` variant and use badge-only, no banner.

2. **DLL injection path validation**: `InjectionSection.dll_paths` are silently skipped in Phase 2. Recommend including in the next phase — the health check function's design should make it easy to add additional field checks.

3. **Opt-in startup health check**: Should background health scanning be opt-in via settings (for users on very slow storage)? Since it's async and non-blocking, always-on may be acceptable. Open for UX/recommendations input.

4. **`Warning` severity for optional paths**: Phase 2 uses only Broken/Stale for issue severity. Should missing-but-optional trainer path produce a Warning (softer) state? Recommend: defer to Phase 3.

5. **Proton auto-update specificity**: When `SteamProtonPathMissing` fires due to Proton auto-update, the message is generic. A future enhancement could detect the pattern (parent directory exists, sub-version gone) and say "Steam updated this Proton version — use Auto-Populate to find the new path." Phase 2: generic message is acceptable.

6. **Re-check after profile save**: Currently proposed: invalidate single profile result on save, user triggers manual full re-check. Alternative: auto-re-validate only the saved profile immediately after save. More responsive UX, slightly more complex. Open for UX input.
