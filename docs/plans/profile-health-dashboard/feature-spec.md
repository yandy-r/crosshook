# Feature Spec: Profile Health Dashboard

## Executive Summary

The profile health dashboard (GitHub #38, Phase 2 diagnostics) adds batch validation of all saved CrossHook profiles, surfacing per-profile health status (healthy/stale/broken) with specific remediation suggestions for broken filesystem paths. The feature is buildable with **zero new Rust dependencies** — approximately 80% of the logic already exists in `validate_all()`, `ValidationError::help()`, `ProfileStore::list()`/`load()`, and the `CompatibilityBadge` UI pattern. The primary new work is a `profile/health.rs` module in `crosshook-core` that validates `GameProfile` path fields directly against the filesystem (distinct from launch-time validation), a thin Tauri command layer, and inline health badges on the profile list using existing CSS patterns. Key risks are `GameProfile → LaunchRequest` conversion divergence (mitigated by comprehensive tests) and user overwhelm from batch warnings (mitigated by aggregate summary UI with progressive disclosure).

---

## External Dependencies

### APIs and Services

**None.** This is a fully local feature with no network calls, no external APIs, and no new crate dependencies. All filesystem checks use `std::fs::metadata()` and `std::os::unix::fs::PermissionsExt`.

### Libraries and SDKs

| Library                             | Version                                | Purpose                                     | Status               |
| ----------------------------------- | -------------------------------------- | ------------------------------------------- | -------------------- |
| `std::fs`                           | stdlib                                 | Path existence, type, and permission checks | Built-in             |
| `std::os::unix::fs::PermissionsExt` | stdlib                                 | Executable bit checking (`mode() & 0o111`)  | Built-in             |
| `tokio`                             | `1.x` (already in Cargo.toml)          | `spawn_blocking` for async startup scan     | Already a dependency |
| `serde`                             | `1.x` (already in Cargo.toml)          | Serialize health results across IPC         | Already a dependency |
| `tempfile`                          | dev-dependency (already in Cargo.toml) | Unit test temp directories                  | Already a dependency |

### External Documentation

- [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/): `app.manage(Mutex<T>)` pattern for health cache
- [Tauri v2 Calling Frontend](https://v2.tauri.app/develop/calling-frontend/): `AppHandle::emit()` for startup event push
- [Tokio fs module](https://docs.rs/tokio/latest/tokio/fs/index.html): Confirms `tokio::fs` uses `spawn_blocking` internally

---

## Business Requirements

### User Stories

**Primary User: Steam Deck user managing multiple game profiles**

- As a Steam Deck user, I want to see a health badge on each profile at a glance so that I know which profiles need attention before game night
- As a Steam Deck user, I want CrossHook to check all profiles in the background at startup so that I get notified of broken configs after Proton auto-updates without any extra steps
- As a user who uninstalled a game, I want affected profiles marked stale rather than broken so that I understand this is a normal lifecycle event

**Secondary User: Linux gamer who imports community profiles**

- As a user who imported a community profile, I want to see immediately if the profile references paths that don't exist on my system so that I can run Auto-Populate before wasting time on a launch attempt
- As a user with many profiles, I want to see a summary count ("3 of 12 profiles have issues") rather than a wall of warnings so that I can triage what matters
- As a user with a broken profile, I want to see the specific path that failed and a fix suggestion so that I know exactly what to do

### Business Rules

1. **Health vs. Launch Validation Boundary**: Health checks validate whether filesystem paths stored in `GameProfile` exist at rest. They do NOT validate launch-configuration compatibility, optimization conflicts, `steam_client_install_path` (derived at runtime), WINE prefix structural validity, or Steam AppID resolution. A profile can be health-healthy and still fail launch validation.

2. **Tri-State Health Classification**:
   - **Healthy** — all required fields configured; all configured paths exist with correct type and permissions
   - **Stale** — required fields configured; one or more configured paths missing from disk (ENOENT). Covers Proton auto-updates, game uninstalls, and unmounted SD cards. This is a normal lifecycle event.
   - **Broken** — required field empty/unconfigured, OR path exists but wrong type, OR path exists but inaccessible (EACCES). Requires user action.

3. **Severity Precedence**: If a profile has both Stale and Broken issues, the overall status is **Broken** (more severe wins).

4. **Method-Aware Validation**: Health checks only validate fields required by the profile's resolved launch method. `steam.proton_path` is only checked for `steam_applaunch`; `runtime.prefix_path` only for `proton_run`. Empty optional fields produce no issue.

5. **Removable Media Rule**: `Path::exists()` returning false is always classified as **Missing → Stale**, regardless of whether the cause is a deleted file or unmounted SD card. The system cannot distinguish these cases; Stale is the correct conservative classification.

6. **Permission Denied Is Distinct**: A file that exists but has `chmod 000` is reported as **Inaccessible → Broken** with remediation "check file permissions" — not conflated with Missing.

7. **No Auto-Repair**: The health dashboard is strictly read-only diagnostic. It classifies and surfaces issues but never modifies profile data. Remediation actions (edit profile, run Auto-Populate, delete profile) are user-initiated.

8. **No Persistence**: Health results are held in frontend state (in-memory) only — never written to disk. Results are invalidated when any profile is saved, renamed, or deleted.

9. **Non-Blocking Startup**: Startup health scan runs as a background async task after UI renders — never in the synchronous `startup.rs` path. A single profile failing to load must not abort the batch scan.

10. **Notification Rules**: Broken profiles → startup banner. Stale profiles → badge only (no banner). Unconfigured profiles → badge only. Dismiss is per-session; re-shows next launch if issues persist.

### Edge Cases

| Scenario                                           | Expected Behavior                                                                                | Notes                                                              |
| -------------------------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------ |
| Empty/unconfigured profile (all paths empty)       | Classified as Broken but presented with softer "Unconfigured" UI tone                            | Badge only, no startup banner; normal for new profiles             |
| Community-imported profile with many missing paths | Show contextual note: "This profile was imported — paths may need to be updated for your system" | Must-have to avoid blaming CrossHook                               |
| Profile TOML parse error                           | Classified as Broken with "Profile data could not be read" message                               | Must not abort batch scan                                          |
| SD card unmounted (Steam Deck)                     | All affected profiles show Stale, not Broken                                                     | Conservative classification; no way to detect unmount vs. deletion |
| Proton auto-updated by Steam (e.g., 9.0-1 → 9.0-2) | Profile shows Stale (old path missing)                                                           | Future: detect pattern for targeted "Proton updated" message       |
| Symlink to deleted target                          | Reports as Missing (Stale) — `metadata()` follows symlinks                                       | Correct behavior                                                   |
| File exists but not executable (Proton binary)     | Reports as Broken (WrongType)                                                                    | `PermissionsExt::mode() & 0o111 == 0`                              |

### Success Criteria

- [x] All saved profiles can be validated in batch
- [x] Each profile shows a health status indicator (healthy/stale/broken)
- [x] Broken paths are identified with specific remediation suggestions
- [x] Health check can be triggered manually and runs on app startup

---

## Technical Specifications

### Architecture Overview

```text
┌─────────────────────────────────────────────────────────┐
│  React Frontend                                         │
│  ┌──────────────────┐  ┌───────────────────────────┐    │
│  │ Profile List +    │  │ HealthBadge (reuses       │    │
│  │ Health Badges     │  │ crosshook-status-chip)    │    │
│  └────────┬─────────┘  └───────────────────────────┘    │
│           │ invoke()                                     │
│  ┌────────┴─────────┐  listen("profile-health-batch-    │
│  │ useProfileHealth  │◄──── complete")                   │
│  │ (new hook)        │                                   │
│  └────────┬─────────┘                                   │
└───────────┼─────────────────────────────────────────────┘
            │ Tauri IPC
┌───────────┼─────────────────────────────────────────────┐
│  src-tauri│                                             │
│  ┌────────┴─────────┐                                   │
│  │ commands/         │  batch_validate_profiles()        │
│  │ profile.rs        │  get_profile_health(name)         │
│  │ (extend existing) │  sanitize_display_path() on       │
│  └────────┬─────────┘  all path fields                  │
└───────────┼─────────────────────────────────────────────┘
            │
┌───────────┼─────────────────────────────────────────────┐
│  crosshook-core                                         │
│  ┌────────┴─────────┐  ┌───────────────────────────┐    │
│  │ profile/health.rs │  │ profile/models.rs          │    │
│  │  ProfileHealthInfo│──│  GameProfile fields         │    │
│  │  check_profile_   │  │ profile/toml_store.rs       │    │
│  │  health()         │  │  list() + load()            │    │
│  │  batch_check_     │  └───────────────────────────┘    │
│  │  health()         │                                   │
│  └──────────────────┘  uses: std::fs::metadata()         │
└─────────────────────────────────────────────────────────┘
```

### Data Models

#### Rust Structs (`crates/crosshook-core/src/profile/health.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileHealthStatus {
    Healthy,
    Stale,
    Broken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueKind {
    NotConfigured,
    Missing,
    Inaccessible,
    WrongType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileHealthIssue {
    pub field: String,       // "game.executable_path", "steam.proton_path"
    pub path: String,        // sanitized display path (~/...) or empty if unconfigured
    pub message: String,     // what went wrong
    pub help: String,        // remediation suggestion
    pub kind: HealthIssueKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileHealthResult {
    pub name: String,
    pub status: ProfileHealthStatus,
    pub launch_method: String,
    pub issues: Vec<ProfileHealthIssue>,
    pub checked_at: String,  // ISO 8601
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    pub profiles: Vec<ProfileHealthResult>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}
```

**Status derivation from issue kinds:**

- `Missing` → contributes to Stale
- `NotConfigured` (required field) / `Inaccessible` / `WrongType` → contributes to Broken
- `NotConfigured` (optional field) → no contribution (expected)

#### TypeScript Interfaces (`src/types/health.ts`)

```typescript
export type ProfileHealthStatus = 'healthy' | 'stale' | 'broken';
export type HealthIssueKind = 'not_configured' | 'missing' | 'inaccessible' | 'wrong_type';

export interface ProfileHealthIssue {
  field: string;
  path: string;
  message: string;
  help: string;
  kind: HealthIssueKind;
}

export interface ProfileHealthResult {
  name: string;
  status: ProfileHealthStatus;
  launch_method: string;
  issues: ProfileHealthIssue[];
  checked_at: string;
}

export interface HealthCheckSummary {
  profiles: ProfileHealthResult[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}
```

### API Design

#### `batch_validate_profiles` — Tauri Command

**Purpose**: Validate all saved profiles and return aggregate health summary.
**Input**: None.
**Response**: `HealthCheckSummary`.
**Errors**: Stringified error if `ProfileStore::list()` fails. Individual profile load failures are captured as Broken entries in the summary (not command-level errors).

```typescript
const summary = await invoke<HealthCheckSummary>('batch_validate_profiles');
```

#### `get_profile_health` — Tauri Command

**Purpose**: Validate a single profile by name.
**Input**: `{ name: string }`.
**Response**: `ProfileHealthResult`.
**Errors**: Stringified `ProfileStoreError` if profile does not exist or cannot be parsed.

```typescript
const report = await invoke<ProfileHealthResult>('get_profile_health', { name });
```

#### `profile-health-batch-complete` — Tauri Event (startup)

**Purpose**: Push startup health results to frontend after background scan.
**Payload**: `HealthCheckSummary`.
**Timing**: Emitted ~500ms after UI renders via async task.

```typescript
import { listen } from '@tauri-apps/api/event';
const unlisten = await listen<HealthCheckSummary>('profile-health-batch-complete', (event) => {
  setHealthResults(event.payload);
});
```

### System Integration

#### Files to Create

| File                                          | Purpose                                                                                                                                                         |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/health.rs` | Core health validation: `ProfileHealthStatus`, `ProfileHealthResult`, `ProfileHealthIssue`, `HealthIssueKind`, `check_profile_health()`, `batch_check_health()` |
| `src/hooks/useProfileHealth.ts`               | React hook for health state management (mirrors `useLaunchState` reducer pattern)                                                                               |
| `src/components/HealthBadge.tsx`              | Reusable status badge (follows `CompatibilityBadge` pattern)                                                                                                    |
| `src/types/health.ts`                         | TypeScript type definitions                                                                                                                                     |

#### Files to Modify

| File                                          | Change                                                                                             |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/mod.rs`    | Add `pub mod health;`                                                                              |
| `crates/crosshook-core/src/launch/request.rs` | Promote `require_directory()`, `require_executable_file()`, `is_executable_file()` to `pub(crate)` |
| `src-tauri/src/commands/profile.rs`           | Add `batch_validate_profiles` and `get_profile_health` Tauri commands                              |
| `src-tauri/src/lib.rs`                        | Register new commands in `invoke_handler`; optionally spawn startup health check                   |
| `src/types/index.ts`                          | Add `export * from './health';`                                                                    |
| `src/components/ProfileEditor.tsx`            | Add inline health badges to profile selector list                                                  |
| `src/App.tsx`                                 | Integrate health badge display in profile list area                                                |

#### Configuration

No new Tauri capabilities required. `std::fs::metadata()` does not require the `fs:read` plugin — Rust-side I/O in Tauri commands is unrestricted.

---

## UX Considerations

### User Workflows

#### Primary Workflow: Startup Health Check

1. **App starts** — Profile list renders immediately with no health badges
2. **Background scan** — Async task validates all profiles after UI ready
3. **Results arrive** — `profile-health-batch-complete` event fires; all badges update atomically
4. **Broken notification** — If ≥1 profile is Broken, startup banner appears: "N profiles have broken paths" [Review]
5. **Stale/healthy** — Badge only, no banner (stale is expected lifecycle noise)

#### Primary Workflow: Manual Health Check

1. **User clicks "Re-check All"** — Loading state shown ("Checking profiles...")
2. **Frontend invokes `batch_validate_profiles`** — Synchronous, <500ms typical
3. **Results replace cache** — All badges and summary count update
4. **If issues persist** — Summary refreshed, banner re-shown

#### Primary Workflow: Drill-Down to Broken Profile

1. **User selects broken profile** (D-pad Down + Confirm on Steam Deck)
2. **Detail panel expands inline** — `CollapsibleSection` shows per-issue list
3. **Each issue shows**: field label, message, remediation help text, affected path (sanitized)
4. **Single CTA**: "Open Profile" navigates to `ProfileEditor`
5. **On save** → auto-revalidate that single profile via `get_profile_health`

#### Error Recovery Workflow

1. **Error**: Profile TOML is corrupt / unparseable
2. **User sees**: Profile badge shows Broken with "Profile data could not be read"
3. **Recovery**: "Try deleting and re-creating this profile"

### UI Patterns

| Component      | Pattern                                                                      | Notes                                                                       |
| -------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Health badge   | `crosshook-status-chip crosshook-compatibility-badge--{rating}`              | Reuse existing CSS class; map healthy→working, stale→partial, broken→broken |
| Issue detail   | `CollapsibleSection` with per-issue list                                     | Already used in `CompatibilityViewer` and `LaunchPanel`                     |
| Summary banner | `crosshook-rename-toast` pattern with `role="status"` + `aria-live="polite"` | Dismissible, non-modal                                                      |
| Severity icons | `severityIcon()` function (extract to `src/utils/severity.ts`)               | Pure 5-line lookup, justified deduplication                                 |
| Loading state  | Spinner badge (`unchecked`) during validation                                | All badges render atomically on batch complete                              |

### Accessibility Requirements

- **Color + icon + text label** on every badge (never rely on color alone) — already the pattern in `CompatibilityBadge`
- **Minimum touch target**: `--crosshook-touch-target-min: 48px` for all Recheck and Fix buttons
- **Focus management**: `useGamepadNav` two-zone model; health detail content zone; profile cards as focusable units
- **Controller hints**: `ControllerPrompts` overlay shows "Y Re-check" / "A Open" when broken profile is focused
- **`scrollIntoView({ block: 'nearest' })`** when gamepad navigates to health badges

### Performance UX

- **Loading States**: All profile badges show `[Checking…]` spinner during validation; update atomically on completion
- **Batch timing**: <50ms typical, up to 2s on Steam Deck SD card — acceptable for async non-blocking
- **Optimistic Updates**: On single-profile recheck, immediately show spinner; keep previous status visible with "checking..." overlay until result arrives
- **Silent Success**: No notification when all profiles are healthy

---

## Recommendations

### Implementation Approach

**Recommended Strategy**: Inline health badges on the existing profile list (not a new tab/page). Build on existing `validate_all()` path-checking infrastructure, `CompatibilityBadge` CSS pattern, and `CollapsibleSection` detail panels. Start synchronous and simple; add async startup scan as the final phase.

**Phasing:**

1. **Phase A — Core Health Check (MVP, 3-5 days)**: `profile/health.rs` module, Tauri commands, inline badges on profile list, `useProfileHealth` hook, per-issue remediation hints
2. **Phase B — Polish (1-2 days)**: Health detail with `CollapsibleSection`, DLL/icon path checks, ENOENT/EACCES distinction in remediation text, filter/sort by status
3. **Phase C — Startup Integration (0.5-1 day)**: Always-on non-blocking startup scan via Tauri event, summary banner for broken profiles
4. **Phase D — Downstream**: Feed into #49 diagnostic bundle, #48 Proton migration, #64 stale launcher detection

### Technology Decisions

| Decision            | Recommendation                                       | Rationale                                                                                                                                             |
| ------------------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| New module location | `profile/health.rs` (not top-level `health/`)        | Health checking is a profile-domain concern; one new file sufficient                                                                                  |
| Batch strategy      | Synchronous via `spawn_blocking`                     | 50 profiles × 8 paths × ~1ms = ~400ms. Simpler than async alternatives                                                                                |
| Health issue type   | New `ProfileHealthIssue` with `HealthIssueKind` enum | Provides machine-readable issue classification for UI; `LaunchValidationIssue` lacks `field` and `kind` discriminants needed for targeted remediation |
| Path checking       | `std::fs::metadata()` (not `Path::exists()`)         | Returns `Result` that distinguishes NotFound from PermissionDenied                                                                                    |
| Caching             | No server-side cache; frontend-only state            | Filesystem state changes at any time; checks are fast enough to re-run                                                                                |
| File watching       | Reject `notify` crate                                | Wrong trigger model for on-demand/startup checks; adds unnecessary complexity                                                                         |
| Parallel validation | Reject `rayon`                                       | I/O-bound, not CPU-bound; `rayon` is wrong fit                                                                                                        |

### Quick Wins

- **Reuse `CompatibilityBadge` CSS** for health chips — minutes of work, proven pattern
- **Reuse `CollapsibleSection`** for detail panels — already in `LaunchPanel` and `CompatibilityViewer`
- **Reuse `ValidationError::help()` text** for remediation — zero new copy needed for existing error variants
- **Reuse `sanitize_display_path()`** for path display — already strips `$HOME` to `~`
- **Reuse `ProfileStore::with_base_path()`** for testing — create temp dir stores, no mocking

### Future Enhancements

- **Auto-repair for Proton updates**: Detect "parent directory exists, sub-version gone" pattern and suggest one-click update via Auto-Populate
- **File watching (inotify)**: Only if users report confusion about stale health status; not needed for on-demand model
- **Health history**: Track validation timestamps over time; power-user feature
- **Batch repair**: "Fix all stale Proton paths" button; natural extension of #48 Proton migration
- **CLI health command**: `crosshook health` trivial to wire since logic is in `crosshook-core`; Phase 5 (#43)

---

## Risk Assessment

### Technical Risks

| Risk                                                     | Likelihood | Impact                                      | Mitigation                                                                                                                                                                             |
| -------------------------------------------------------- | ---------- | ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Health check path logic diverges from launch validation  | Medium     | High — false health results                 | Promote `require_directory()`, `require_executable_file()`, `is_executable_file()` to `pub(crate)`; share path-checking primitives between both validation paths. Comprehensive tests. |
| Batch validation I/O blocks Tauri main thread            | Low        | Medium — UI freeze                          | Start synchronous (~400ms acceptable). Use `tokio::task::spawn_blocking` only if profiling reveals latency.                                                                            |
| Profile TOML parse errors crash batch scan               | Medium     | Medium — one bad profile breaks all results | Catch `ProfileStoreError` per-profile, report as Broken entry. Never propagate with `?` from per-profile loop.                                                                         |
| Empty profiles classified as Broken alarm new users      | Medium     | Medium — bad first impression               | Detect "all NotConfigured" as Unconfigured variant; use badge-only (no banner).                                                                                                        |
| Community-imported profiles appear immediately broken    | Medium     | Low — expected but confusing                | Show "This profile was imported — use Auto-Populate to configure" instead of generic Broken.                                                                                           |
| Steam Deck SD card profiles appear broken when unmounted | Medium     | Low — user confusion                        | Hardened business rule: missing path = always Stale, never Broken.                                                                                                                     |

### Integration Challenges

- **`LaunchValidationIssue` reuse vs. new type**: Research revealed that `LaunchValidationIssue` lacks `field` and `kind` discriminants needed for targeted remediation UI. New `ProfileHealthIssue` type recommended, but reuse help text from `ValidationError::help()`.
- **Startup race condition**: Emitting Tauri events before frontend listener registration loses events. Mitigation: frontend calls `invoke('batch_validate_profiles')` on mount, not Rust-side auto-emit.
- **Path sanitization at IPC boundary**: All path strings must pass through `sanitize_display_path()` before crossing IPC. Move this function to `src-tauri/src/commands/shared.rs` for shared use.

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| Finding                                                | Risk                                                      | Mitigation                                                                  | Alternatives                                                    |
| ------------------------------------------------------ | --------------------------------------------------------- | --------------------------------------------------------------------------- | --------------------------------------------------------------- |
| W-1: CSP disabled (`"csp": null` in `tauri.conf.json`) | XSS could probe arbitrary paths via new IPC commands      | Enable CSP: `"csp": "default-src 'self'; script-src 'self'"`                | Document as tech debt with local-app threat model justification |
| W-2: Raw paths in IPC responses                        | Leaks filesystem layout in logs or future crash reporting | Apply `sanitize_display_path()` to all path fields before IPC serialization | Return enum-tagged field types without raw paths                |
| W-3: Diagnostic bundle path leak (#49 downstream)      | Health reports exported in bundle expose filesystem       | Sanitize all paths before export; document as hard dependency for #49       | —                                                               |

#### Advisories — Best Practices

- **A-1: Distinguish ENOENT vs. EACCES**: Use `std::fs::metadata()` error kinds, not `Path::exists()` (deferral: Phase B acceptable)
- **A-2: Symlink following is correct**: Document that `metadata()` follows symlinks; this is intended behavior
- **A-3: TOCTOU is inherent**: Health check is advisory-only; display "checked at" timestamp (deferral: acceptable for status-only feature)
- **A-4: Batch concurrency**: Sequential validation is sufficient; bound to 4 concurrent only if >100 profiles (deferral: profile if needed)
- **A-5: IPC result type**: Prefer structured enum-tagged fields over raw path strings (deferral: apply `sanitize_display_path()` as minimum)

---

## Task Breakdown Preview

### Pre-Ship Security

**Focus**: Address security warnings before expanding IPC surface.
**Tasks**:

- Enable CSP in `tauri.conf.json` (W-1) — one-line change + testing
- Move `sanitize_display_path()` to `src-tauri/src/commands/shared.rs` (W-2)

### Phase A: Core Health Check (MVP)

**Focus**: Batch validation, inline badges, remediation hints.
**Tasks**:

1. Promote `require_directory()`, `require_executable_file()`, `is_executable_file()` to `pub(crate)` in `request.rs`
2. Create `ProfileHealthStatus`, `ProfileHealthResult`, `ProfileHealthIssue`, `HealthIssueKind` types in new `profile/health.rs`
3. Implement `check_profile_health()` — method-aware path validation on `GameProfile` fields directly
4. Implement `batch_check_health()` — iterate `ProfileStore::list()`/`load()`, catch per-profile errors as Broken
5. Write Rust unit tests using `tempfile::tempdir()` + `ProfileStore::with_base_path()` pattern
6. Add `batch_validate_profiles` and `get_profile_health` Tauri commands (sanitize paths before return)
7. Create TypeScript types in `src/types/health.ts`
8. Create `useProfileHealth` hook (mirrors `useLaunchState` reducer pattern)
9. Create `HealthBadge` component (reuse `crosshook-status-chip` CSS)
10. Add inline health badges to profile selector list
11. Add per-issue remediation detail with `CollapsibleSection`
12. Hook into `save_profile` to auto-revalidate saved profile

**Parallelization**: Tasks 1-5 (Rust) can run in parallel with tasks 7-9 (TypeScript types + hook + component). Tasks 6 and 10-12 depend on both.

### Phase B: Detail & Remediation Polish

**Focus**: Enhanced UX and additional path checks.
**Dependencies**: Phase A complete.
**Tasks**:

- Distinguish ENOENT vs. EACCES for remediation text (security A-1)
- Add DLL injection path checks (Warning severity)
- Add icon path checks (Info severity)
- Add filter/sort profiles by health status
- Add "Unconfigured" detection for brand-new profiles
- Add community-import context note

### Phase C: Startup Integration

**Focus**: Always-on background validation at startup.
**Dependencies**: Phase A complete.
**Tasks**:

- Spawn non-blocking async health check ~500ms after UI renders
- Emit `profile-health-batch-complete` Tauri event
- Add startup summary banner for broken profiles (non-modal, dismissible)
- Extract `severityIcon()` to `src/utils/severity.ts` if needed by health dashboard

---

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **Health Status Labels**
   - Options: "Healthy / Stale / Broken" (developer-oriented) vs. "Healthy / Needs Attention / Cannot Launch" (user-friendly)
   - Impact: Affects badge text, notifications, and documentation
   - Recommendation: Use "Healthy / Stale / Broken" in code/types; user-facing labels can be mapped in the component

2. **`LaunchRequest` Conversion vs. Direct `GameProfile` Validation**
   - Options: (A) Build `GameProfile::to_launch_request()` and reuse `validate_all()`, (B) Validate `GameProfile` fields directly in new health module
   - Impact: Option A has conversion complexity + `steam_client_install_path` injection issue. Option B has duplication risk but simpler dependency chain.
   - Recommendation: **Option B** — validate `GameProfile` directly, share path-checking primitives via `pub(crate)`. Business-analyzer and practices-researcher aligned on this approach. Eliminates `steam_client_install_path` injection problem entirely.

---

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Tokio/Tauri library analysis, batch validation patterns, file system considerations, `notify` crate evaluation (rejected)
- [research-business.md](./research-business.md): User stories, health classification rules, method-aware validation, remediation text, notification rules, workflow definitions
- [research-technical.md](./research-technical.md): Architecture design, complete Rust struct + TypeScript interface definitions, Tauri command contracts, validation logic pseudocode
- [research-ux.md](./research-ux.md): Competitive analysis (Steam/Lutris/Heroic/Grafana), gamepad navigation constraints, progressive disclosure patterns, error handling UX
- [research-security.md](./research-security.md): 0 critical, 3 warnings (CSP, path sanitization, diagnostic bundle), 5 advisories. Local desktop threat model assessment.
- [research-practices.md](./research-practices.md): Reusable code inventory (11 items), KISS assessment, module boundary recommendation, testability patterns, build-vs-depend analysis
- [research-recommendations.md](./research-recommendations.md): Phased implementation plan, risk assessment matrix, resolved team decisions, edge case handling, downstream feature integration
