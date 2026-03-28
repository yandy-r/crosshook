# Feature Spec: Profile Health Dashboard (v2)

## Executive Summary

The profile health dashboard (GitHub #38, Phase 2 diagnostics) adds batch validation of all saved CrossHook profiles, surfacing per-profile health status (healthy/stale/broken) with specific remediation suggestions for broken filesystem paths. **v2 revision**: With the SQLite metadata layer now implemented (PRs 89-91), health results can be enriched with launch history failure trends (`query_failure_trends()`), last-success timestamps (`query_last_success_per_profile()`), launcher drift detection (`launcher_sync.drift_state`), and optional persistence for startup display — all via existing `MetadataStore` APIs requiring zero new tables in Phase A/B. The feature uses a **two-layer architecture**: a pure-filesystem core health module in `profile/health.rs` (no MetadataStore dependency, testable with tempdir), and a Tauri command enrichment layer in `commands/health.rs` that composes metadata signals when available. The fail-soft pattern ensures health checks work when MetadataStore is disabled. Zero new Rust dependencies are needed. Primary risks are `GameProfile → LaunchRequest` conversion divergence and composite health scoring ambiguity (filesystem-healthy but launch-failing profiles).

---

## External Dependencies

### APIs and Services

**None.** This is a fully local feature with no network calls and no new crate dependencies. All filesystem checks use `std::fs::metadata()` and `std::os::unix::fs::PermissionsExt`. SQLite integration uses the existing `rusqlite` 0.38.0 (bundled SQLite 3.51.1) already in `Cargo.toml`.

### Libraries and SDKs

| Library                             | Version                                | Purpose                                           | Status               |
| ----------------------------------- | -------------------------------------- | ------------------------------------------------- | -------------------- |
| `std::fs`                           | stdlib                                 | Path existence, type, and permission checks       | Built-in             |
| `std::os::unix::fs::PermissionsExt` | stdlib                                 | Executable bit checking (`mode() & 0o111`)        | Built-in             |
| `tokio`                             | `1.x` (already in Cargo.toml)          | `spawn_blocking` for async startup scan           | Already a dependency |
| `serde`                             | `1.x` (already in Cargo.toml)          | Serialize health results across IPC               | Already a dependency |
| `chrono`                            | already in Cargo.toml                  | `Utc::now().to_rfc3339()` for `checked_at` stamps | Already a dependency |
| `rusqlite`                          | `0.38.0` (already in Cargo.toml)       | MetadataStore queries for health enrichment       | Already a dependency |
| `tempfile`                          | dev-dependency (already in Cargo.toml) | Unit test temp directories                        | Already a dependency |

### External Documentation

- [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/): `app.manage(Mutex<T>)` pattern for health cache
- [Tauri v2 Calling Frontend](https://v2.tauri.app/develop/calling-frontend/): `AppHandle::emit()` for startup event push
- [SQLite WAL](https://www.sqlite.org/wal.html): existing metadata DB journaling mode
- [rusqlite docs](https://docs.rs/rusqlite/latest/rusqlite/): query API for health enrichment

---

## Business Requirements

### User Stories

**Primary User: Steam Deck user managing multiple game profiles**

- US-1: As a Steam Deck user, I want to see a health badge on each profile at a glance so that I know which profiles need attention before game night
- US-2: As a Steam Deck user, I want CrossHook to check all profiles in the background at startup so that I get notified of broken configs after Proton auto-updates without any extra steps
- US-6: As a user who uninstalled a game, I want affected profiles marked stale rather than broken so that I understand this is a normal lifecycle event

**Secondary User: Linux gamer who imports community profiles**

- US-5: As a user who imported a community profile, I want to see immediately if the profile references paths that don't exist on my system so that I can run Auto-Populate before wasting time on a launch attempt
- US-3: As a user with many profiles, I want to see a summary count ("3 of 12 profiles have issues") rather than a wall of warnings so that I can triage what matters
- US-4: As a user with a broken profile, I want to see the specific path that failed and a fix suggestion so that I know exactly what to do

**[NEW] Metadata-Enriched Stories**

- US-8: As a user with recurring failures, I want to see which profiles have been failing recently even when paths look intact so that I can investigate configuration issues that don't manifest as missing files
- US-9: As a user with exported launchers, I want to see if any of my exported launchers have drifted from their source profiles so that I know if my desktop shortcuts are stale
- US-10: As a power user, I want to see a profile's last successful launch date alongside its health badge so that I can prioritize fixing profiles I actually use
- US-11: As a user browsing favorites, I want to filter the health view by my favorite profiles or a collection so that I can quickly assess just the profiles I care most about

### Business Rules

1. **Health vs. Launch Validation Boundary**: Health checks validate whether filesystem paths stored in `GameProfile` exist at rest. They do NOT validate launch-configuration compatibility, optimization conflicts, `steam_client_install_path` (derived at runtime), WINE prefix structural validity, or Steam AppID resolution. A profile can be health-healthy and still fail launch validation.

2. **Tri-State Health Classification**:
   - **Healthy** — all required fields configured; all configured paths exist with correct type and permissions
   - **Stale** — required fields configured; one or more configured paths missing from disk (ENOENT). Covers Proton auto-updates, game uninstalls, and unmounted SD cards. This is a normal lifecycle event.
   - **Broken** — required field empty/unconfigured, OR path exists but wrong type, OR path exists but inaccessible (EACCES). Requires user action.

3. **Severity Precedence**: Broken > Stale > Degraded > Healthy.

4. **Method-Aware Validation**: Health checks only validate fields required by the profile's resolved launch method. `steam.proton_path` is only checked for `steam_applaunch`; `runtime.prefix_path` only for `proton_run`. Empty optional fields produce no issue.

5. **Removable Media Rule**: `Path::exists()` returning false is always classified as **Missing → Stale**, regardless of whether the cause is a deleted file or unmounted SD card.

6. **Permission Denied Is Distinct**: A file that exists but has `chmod 000` is reported as **Inaccessible → Broken** with remediation "check file permissions" — not conflated with Missing. Uses `std::fs::metadata()` error kinds.

7. **No Auto-Repair**: The health dashboard is strictly read-only diagnostic. It classifies and surfaces issues but never modifies profile data.

8. **[REVISED] Persistence**: Health results CAN be persisted to SQLite via `MetadataStore`, keyed by the profile's stable `profile_id` UUID. Persistence is optional and additive — the filesystem scan remains the authoritative source of truth. **Phase A/B: no persistence (frontend-only state). Phase D: optional persistence via `health_snapshots` table (migration v6).**

9. **Non-Blocking Startup**: Startup health scan runs as a background async task after UI renders — never in the synchronous `startup.rs` path. A single profile failing to load must not abort the batch scan.

10. **Notification Rules**: Broken profiles → startup banner. Stale profiles → badge only (no banner). Unconfigured profiles → badge only. Degraded profiles → badge only (no banner). Dismiss is per-session; re-shows next launch if issues persist.

11. **[NEW] Composite Health Signal (BR-NEW-1)**: A profile can be filesystem-healthy but launch-failing. "Degraded" is a composite sub-state of Healthy: paths exist, but `query_failure_trends()` shows ≥2 failures and 0 successes in the last 30 days. Degraded maps to amber badge with a failure count indicator. `clean_exit` outcomes are not counted as failures.

12. **[NEW] Launcher Drift (BR-NEW-3)**: Launcher drift is a **separate health dimension** surfaced as a secondary indicator, not a modifier of the primary health badge. `DriftState::Missing` or `Moved` or `Stale` → amber drift warning. `Aligned` or `Unknown` → no indicator.

13. **[NEW] Fail-Soft Degradation (BR-NEW-4)**: When `MetadataStore.available` is false, all metadata enrichment is silently skipped. Health falls back to filesystem-only checks. No error banner for missing metadata. Degradation boundaries:
    - Filesystem path checks: always available
    - Launch failure trend enrichment: omitted when unavailable
    - Last-success timestamps: omitted when unavailable
    - Launcher drift indicator: omitted when unavailable
    - Collection/favorites filter: controls hidden or disabled

### Edge Cases

| Scenario                                              | Expected Behavior                                                                                | Notes                                                        |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| Empty/unconfigured profile (all paths empty)          | Classified as Broken but presented with softer "Unconfigured" UI tone                            | Badge only, no startup banner; normal for new profiles       |
| Community-imported profile with many missing paths    | Show contextual note: "This profile was imported — paths may need to be updated for your system" | Must-have to avoid blaming CrossHook                         |
| Profile TOML parse error                              | Classified as Broken with "Profile data could not be read" message                               | Must not abort batch scan                                    |
| SD card unmounted (Steam Deck)                        | All affected profiles show Stale, not Broken                                                     | Conservative classification                                  |
| Proton auto-updated by Steam (e.g., 9.0-1 → 9.0-2)    | Profile shows Stale (old path missing)                                                           | Future: detect pattern for targeted "Proton updated" message |
| Symlink to deleted target                             | Reports as Missing (Stale) — `metadata()` follows symlinks                                       | Correct behavior                                             |
| File exists but not executable (Proton binary)        | Reports as Broken (WrongType)                                                                    | `PermissionsExt::mode() & 0o111 == 0`                        |
| [NEW] Profile renamed — health result in SQLite       | `profile_id` UUID is rename-stable; persisted result still linked                                | `observe_profile_rename()` invalidates cached result         |
| [NEW] MetadataStore unavailable                       | Health scan runs filesystem-only; no enrichment                                                  | Silent degradation — no error banner                         |
| [NEW] Profile has 30 failures, 0 successes (Degraded) | Shows Amber badge with "Launch failures detected" sub-indicator                                  | Does not trigger startup banner                              |
| [NEW] Launcher drift = Missing for healthy profile    | Amber drift indicator shown alongside Green health badge                                         | Separate dimension — does not change primary badge color     |

### Success Criteria

- [ ] All saved profiles can be validated in batch
- [ ] Each profile shows a health status indicator (healthy/stale/broken)
- [ ] Broken paths are identified with specific remediation suggestions
- [ ] Health check can be triggered manually and runs on app startup
- [ ] [NEW] Profiles with persistent launch failures are flagged (Degraded indicator)
- [ ] [NEW] Last successful launch date shown per profile
- [ ] [NEW] Exported launcher drift surfaced in health view
- [ ] [NEW] Feature degrades gracefully when MetadataStore unavailable

---

## Technical Specifications

### Architecture Overview

```text
┌─────────────────────────────────────────────────────────────────────┐
│  React Frontend                                                     │
│  ┌──────────────────────┐  ┌─────────────────────────┐              │
│  │ ProfileHealthDashboard│  │ HealthBadge             │              │
│  │  (new component)      │  │  (reusable badge)       │              │
│  └──────────┬───────────┘  └─────────────────────────┘              │
│             │ invoke()                                               │
│  ┌──────────┴───────────┐                                           │
│  │ useProfileHealth     │  listen("profile-health-batch-complete")  │
│  │  (new hook)          │◄──────────────────────────────────────────│
│  └──────────┬───────────┘                                           │
└─────────────┼───────────────────────────────────────────────────────┘
              │ Tauri IPC
┌─────────────┼───────────────────────────────────────────────────────┐
│  src-tauri  │                                                       │
│  ┌──────────┴───────────┐  Layer 2: Enrichment (MetadataStore)     │
│  │ commands/health.rs   │  Accepts ProfileStore + MetadataStore     │
│  │  batch_validate_     │  Enriches core results with metadata      │
│  │  profiles()          │  Follows fail-soft pattern                │
│  │  get_profile_health()│                                           │
│  └──────────┬───────────┘                                           │
└─────────────┼───────────────────────────────────────────────────────┘
              │
┌─────────────┼───────────────────────────────────────────────────────┐
│  crosshook-core         │  Layer 1: Core Health (pure filesystem)  │
│  ┌──────────┴───────────┐  ┌─────────────────────────┐              │
│  │ profile/health.rs    │  │ profile/                │              │
│  │  HealthStatus        │──│  models.rs (GameProfile) │              │
│  │  ProfileHealthReport │  │  toml_store.rs (Store)   │              │
│  │  HealthIssue         │  └─────────────────────────┘              │
│  │  check_profile_      │                                           │
│  │  health()            │  ┌─────────────────────────┐              │
│  │  batch_check_        │  │ metadata/ (existing)    │              │
│  │  health()            │  │  query_failure_trends() │              │
│  └──────────────────────┘  │  query_last_success_..()│              │
│                            │  [Phase D: health_store]│              │
│    uses: std::fs::metadata │                          │              │
│    uses: PermissionsExt    └─────────────────────────┘              │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Design: Two-Layer Architecture**

- **Layer 1 (crosshook-core)**: `profile/health.rs` — types and filesystem validation. No MetadataStore dependency. Testable with `ProfileStore::with_base_path()` + `tempdir`.
- **Layer 2 (src-tauri)**: `commands/health.rs` — orchestrates ProfileStore + MetadataStore. Calls Layer 1 for filesystem validation, then enriches with metadata when available. Fail-soft: `metadata: null` when MetadataStore unavailable.

### Data Models

#### Core Health Types (`crates/crosshook-core/src/profile/health.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Stale,
    Broken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueSeverity {
    Error,    // Broken conditions
    Warning,  // Stale conditions
    Info,     // Advisory
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthIssue {
    pub field: String,        // "game.executable_path", "steam.proton_path"
    pub path: String,         // sanitized display path (~/...) or empty if unconfigured
    pub message: String,      // what went wrong
    pub remediation: String,  // fix suggestion
    pub severity: HealthIssueSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileHealthReport {
    pub name: String,
    pub status: HealthStatus,
    pub launch_method: String,
    pub issues: Vec<HealthIssue>,
    pub checked_at: String,   // ISO 8601
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    pub profiles: Vec<ProfileHealthReport>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}
```

#### Metadata-Enriched Types (`src-tauri/src/commands/health.rs`)

```rust
/// Optional metadata enrichment — null when MetadataStore unavailable.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileHealthMetadata {
    pub profile_id: Option<String>,
    pub last_success: Option<String>,       // ISO 8601
    pub failure_count_30d: i64,
    pub total_launches: i64,
    pub launcher_drift_state: Option<String>, // aligned/missing/moved/stale
    pub is_community_import: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnrichedProfileHealthReport {
    #[serde(flatten)]
    pub core: ProfileHealthReport,
    pub metadata: Option<ProfileHealthMetadata>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnrichedHealthSummary {
    pub profiles: Vec<EnrichedProfileHealthReport>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}
```

#### TypeScript Interfaces (`src/types/health.ts`)

```typescript
export type HealthStatus = 'healthy' | 'stale' | 'broken';
export type HealthIssueSeverity = 'error' | 'warning' | 'info';

export interface HealthIssue {
  field: string;
  path: string;
  message: string;
  remediation: string;
  severity: HealthIssueSeverity;
}

export interface ProfileHealthReport {
  name: string;
  status: HealthStatus;
  launch_method: string;
  issues: HealthIssue[];
  checked_at: string;
}

export interface ProfileHealthMetadata {
  profile_id: string | null;
  last_success: string | null;
  failure_count_30d: number;
  total_launches: number;
  launcher_drift_state: string | null;
  is_community_import: boolean;
}

export interface EnrichedProfileHealthReport extends ProfileHealthReport {
  metadata: ProfileHealthMetadata | null;
}

export interface EnrichedHealthSummary {
  profiles: EnrichedProfileHealthReport[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}
```

#### Phase D Schema: `health_snapshots` Table (Migration v6)

```sql
CREATE TABLE IF NOT EXISTS health_snapshots (
    profile_id   TEXT NOT NULL REFERENCES profiles(profile_id),
    status       TEXT NOT NULL,   -- 'healthy', 'stale', 'broken'
    issue_count  INTEGER NOT NULL DEFAULT 0,
    checked_at   TEXT NOT NULL,   -- ISO 8601
    PRIMARY KEY (profile_id)
);
CREATE INDEX IF NOT EXISTS idx_health_snapshots_status ON health_snapshots(status);
```

**Not implemented until Phase D.** One row per profile (UPSERT), bounded storage, FK cascade on profile deletion, no path strings stored.

### API Design

#### `batch_validate_profiles` — Tauri Command

**Purpose**: Validate all saved profiles and return aggregate enriched health summary.
**Input**: None.
**Response**: `EnrichedHealthSummary`.
**Errors**: Stringified error if `ProfileStore::list()` fails. Individual profile load failures are captured as Broken entries (not command-level errors).

```typescript
const summary = await invoke<EnrichedHealthSummary>('batch_validate_profiles');
```

#### `get_profile_health` — Tauri Command

**Purpose**: Validate a single profile by name (for save-triggered revalidation).
**Input**: `{ name: string }`.
**Response**: `EnrichedProfileHealthReport`.
**Errors**: Stringified `ProfileStoreError` if profile does not exist.

```typescript
const report = await invoke<EnrichedProfileHealthReport>('get_profile_health', { name });
```

#### `profile-health-batch-complete` — Tauri Event (startup)

**Purpose**: Push startup health results to frontend after background scan.
**Payload**: `EnrichedHealthSummary`.
**Timing**: Emitted after UI renders via async task.

### System Integration

#### Files to Create

| File                                                           | Purpose                                                                |
| -------------------------------------------------------------- | ---------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/health.rs`                  | Core health types + filesystem validation logic (no MetadataStore dep) |
| `src-tauri/src/commands/health.rs`                             | Tauri IPC: orchestrates ProfileStore + MetadataStore, enriches results |
| `src/hooks/useProfileHealth.ts`                                | React hook for health state management (invoke + listen + useReducer)  |
| `src/components/HealthBadge.tsx`                               | Reusable status badge (follows `CompatibilityBadge` CSS pattern)       |
| `src/components/ProfileHealthDashboard.tsx`                    | Dashboard UI with summary bar + per-profile cards + detail panels      |
| `src/types/health.ts`                                          | TypeScript type definitions                                            |
| [Phase D] `crates/crosshook-core/src/metadata/health_store.rs` | Health snapshot persistence (upsert/load/lookup)                       |

#### Files to Modify

| File                                                         | Change                                                                                             |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/mod.rs`                   | Add `pub mod health;`                                                                              |
| `crates/crosshook-core/src/launch/request.rs`                | Promote `require_directory()`, `require_executable_file()`, `is_executable_file()` to `pub(crate)` |
| `src-tauri/src/commands/mod.rs`                              | Add `pub mod health;`                                                                              |
| `src-tauri/src/lib.rs`                                       | Register health commands in `invoke_handler`; spawn startup health check                           |
| `src/types/index.ts`                                         | Add `export * from './health';`                                                                    |
| `src/App.tsx`                                                | Integrate `ProfileHealthDashboard` inline in profile list area                                     |
| `src/styles/variables.css`                                   | Add health badge CSS custom properties if needed                                                   |
| [Phase D] `crates/crosshook-core/src/metadata/mod.rs`        | Add `mod health_store;` and public methods                                                         |
| [Phase D] `crates/crosshook-core/src/metadata/migrations.rs` | Add `migrate_5_to_6` for `health_snapshots` table                                                  |

#### Configuration

No new Tauri capabilities required. `std::fs::metadata()` does not require the `fs:read` plugin. SQLite operations go through the existing `MetadataStore` already in Tauri state.

---

## UX Considerations

### User Workflows

#### Primary Workflow: Startup Health Check

1. **App starts** — Profile list renders immediately. If MetadataStore available and has cached snapshots from a previous session, show last-known badges. Otherwise show "Not checked yet".
2. **Background scan** — Async task validates all profiles after UI ready. If MetadataStore available, batch-fetches `query_failure_trends(30)` and `query_last_success_per_profile()` before per-profile loop.
3. **Results arrive** — `profile-health-batch-complete` event fires; all badges update atomically.
4. **Broken notification** — If ≥1 profile is Broken, startup banner appears: "N profiles have broken paths" [Review]. Dismissible, non-modal.
5. **Stale/Degraded/healthy** — Badge only, no banner.

#### Primary Workflow: Manual Health Check

1. **User clicks "Re-check All"** — Loading state shown ("Checking profiles...")
2. **Frontend invokes `batch_validate_profiles`** — Synchronous, <500ms typical
3. **Results replace cache** — All badges and summary count update

#### Primary Workflow: Drill-Down to Issue Detail

1. **User selects broken profile** (D-pad Down + Confirm on Steam Deck)
2. **Detail panel expands inline** — `CollapsibleSection` shows per-issue list
3. **Each issue shows**: field label, message, remediation help text, affected path (sanitized)
4. **[NEW] Metadata context**: "Last worked: 3 days ago • 3 failures in last 30 days" when metadata available
5. **[NEW] Launcher drift**: "Exported launcher may be out of sync" when drift detected
6. **Single CTA**: "Open Profile" navigates to `ProfileEditor`

#### Primary Workflow: Auto-Revalidate on Save

1. User saves profile → frontend calls `get_profile_health({ name })`
2. Result updates only that profile's badge in-place (no full batch re-scan)

### UI Patterns

| Component                | Pattern                                                                      | Notes                                                                       |
| ------------------------ | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| Health badge             | `crosshook-status-chip crosshook-compatibility-badge--{rating}`              | Reuse existing CSS class; map healthy→working, stale→partial, broken→broken |
| Failure count indicator  | `↑Nx` overlay on badge                                                       | [NEW] Shows when `failure_count_30d >= 2` and metadata available            |
| Launcher drift indicator | `✦` overlay on badge                                                         | [NEW] Shows when `launcher_drift_state` is missing/moved/stale              |
| Issue detail             | `CollapsibleSection` with per-issue list                                     | Already used in `CompatibilityViewer` and `LaunchPanel`                     |
| Summary banner           | `crosshook-rename-toast` pattern with `role="status"` + `aria-live="polite"` | Dismissible, non-modal                                                      |
| Loading state            | Spinner badge (`unchecked`) during validation                                | All badges render atomically on batch complete                              |
| [NEW] Last-success label | "Last worked: N days ago" in detail panel                                    | Relative timestamp via `checked_at` diff; omitted when metadata unavailable |

### Accessibility Requirements

- **Color + icon + text label** on every badge (never rely on color alone)
- **Minimum touch target**: `--crosshook-touch-target-min: 48px` for all buttons
- **Focus management**: `useGamepadNav` two-zone model; health detail content zone; profile cards as focusable units
- **Controller hints**: `ControllerPrompts` overlay shows "Y Re-check" / "A Open" when broken profile is focused
- **`scrollIntoView({ block: 'nearest' })`** when gamepad navigates to health badges

### Performance UX

- **Loading States**: All profile badges show `[Checking…]` spinner during validation; update atomically on completion
- **Batch timing**: <50ms typical, up to 2s on Steam Deck SD card — acceptable for async non-blocking
- **Metadata enrichment overhead**: ~10-50ms additional for SQL aggregate queries on <50 profiles
- **Silent Success**: No notification when all profiles are healthy

---

## Recommendations

### Implementation Approach

**Recommended Strategy**: Two-layer architecture with phased metadata integration. Build on existing `validate_all()` infrastructure, `CompatibilityBadge` CSS pattern, and `CollapsibleSection` detail panels. Phase A is pure filesystem. Phase B adds metadata enrichment. Phase D adds persistence.

**Phasing:**

1. **Phase A — Core Health Check (MVP, 3-5 days)**: `profile/health.rs` module, Tauri commands, inline badges on profile list, `useProfileHealth` hook, per-issue remediation hints. Zero MetadataStore code.
2. **Phase B — Metadata Enrichment (2-4 days)**: Failure trend badges, "last successful launch" annotations, launcher drift issues, collection-scoped health summaries. Uses existing `MetadataStore` queries — no new SQL, no new tables, no new migrations.
3. **Phase C — Startup Integration (0.5-1 day)**: Always-on non-blocking startup scan via Tauri event, summary banner for broken profiles.
4. **Phase D — Persistence + Trends (1-2 days)**: `health_snapshots` table (migration v6), persist results for instant startup display, trend arrows ("got better/worse").

### Technology Decisions

| Decision                    | Recommendation                                         | Rationale                                                                            |
| --------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| New module location         | `profile/health.rs` (not top-level `health/`)          | Health checking is a profile-domain concern; one new file sufficient                 |
| Batch strategy              | Synchronous via `spawn_blocking`                       | 50 profiles × 8 paths × ~1ms = ~400ms. Simpler than async alternatives               |
| Path checking               | `std::fs::metadata()` (not `Path::exists()`)           | Returns `Result` that distinguishes NotFound from PermissionDenied                   |
| Health issue type           | Reuse `LaunchValidationIssue` where possible           | Practices-researcher: existing `help` text is already remediation guidance           |
| MetadataStore integration   | Fail-soft optional, composition in Tauri command layer | Keeps health module pure-filesystem; matches existing `with_conn` pattern            |
| Health persistence          | Phase D only via `health_snapshots` table              | Phase A needs zero persistence; Phase D is straightforward extension                 |
| Metadata enrichment queries | Use existing APIs, no new SQL                          | `query_failure_trends()`, `query_last_success_per_profile()` are already implemented |
| File watching               | Reject `notify` crate                                  | Wrong trigger model for on-demand/startup checks; adds unnecessary complexity        |
| Parallel validation         | Reject `rayon`                                         | I/O-bound, not CPU-bound; `rayon` is wrong fit                                       |

### Quick Wins

**Existing (zero new code):**

- Reuse `CompatibilityBadge` CSS for health chips — minutes of work
- Reuse `CollapsibleSection` for detail panels
- Reuse `ValidationError::help()` text for remediation
- Reuse `sanitize_display_path()` for path display

**[NEW] Enabled by Metadata Layer:**

- **Failure trend badges**: `query_failure_trends(30)` already implemented — just needs a Tauri command wrapper
- **"Last successful launch" annotation**: `query_last_success_per_profile()` already implemented
- **Launcher drift detection**: `launchers.drift_state` already populated by `launcher_sync`
- **Collection-scoped health**: Compose `list_profiles_in_collection()` + `batch_check_health()` — zero new infrastructure
- **Favorites-only health check**: Same composition pattern

### Future Enhancements

- **Auto-repair for Proton updates**: Detect "parent directory exists, sub-version gone" pattern and suggest update
- **Health history/trends (Phase D)**: With `health_snapshots`, show "got better/worse since last check" trend arrows
- **Profile quality score**: Combine filesystem health + launch success rate + launcher alignment
- **CLI health command**: `crosshook health` — trivial to wire since logic is in `crosshook-core`
- **Batch repair**: "Fix all stale Proton paths" button; natural extension of #48 Proton migration

---

## Risk Assessment

### Technical Risks

| Risk                                                    | Likelihood | Impact | Mitigation                                                                                                                         | v2 Change |
| ------------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------- | --------- |
| Health check path logic diverges from launch validation | Medium     | High   | Promote `require_directory()`, `require_executable_file()`, `is_executable_file()` to `pub(crate)`; share path-checking primitives | No change |
| Batch validation I/O blocks Tauri main thread           | Low        | Medium | Start synchronous (~400ms acceptable). Use `spawn_blocking` only if profiling reveals latency                                      | No change |
| Profile TOML parse errors crash batch scan              | Medium     | Medium | Catch `ProfileStoreError` per-profile, report as Broken entry. Never propagate with `?` from per-profile loop                      | No change |
| Empty profiles classified as Broken alarm new users     | Medium     | Medium | Detect "all NotConfigured" as Unconfigured variant; use badge-only (no banner)                                                     | No change |
| Community-imported profiles appear immediately broken   | Medium     | Low    | Show "This profile was imported — use Auto-Populate to configure"                                                                  | No change |
| [NEW] MetadataStore integration complexity              | Low        | Medium | Follow existing fail-soft pattern; enrichment is additive, never blocking                                                          | New in v2 |
| [NEW] Composite health scoring ambiguity                | Medium     | Medium | Keep separate indicators in Phase B. Do not combine into single score until user feedback validates the need                       | New in v2 |
| [NEW] Migration coupling for Phase D                    | Low        | Low    | Phase A/B have zero migration dependency. Migration v6 is Phase D only                                                             | New in v2 |
| [NEW] Launch history for renamed profiles               | Low        | Low    | Enrichment queries join on `profile_id` (stable UUID) via `lookup_profile_id()`, not `profile_name`                                | New in v2 |

### Integration Challenges

- **`LaunchValidationIssue` reuse vs. new type**: Practices-researcher recommends reusing `LaunchValidationIssue` with the existing `help` text. If a `field: Option<String>` tag is truly needed by the UI, add it to the existing type rather than duplicating.
- **Startup race condition**: Frontend calls `invoke('batch_validate_profiles')` on mount, not Rust-side auto-emit. Avoids event-before-listener race.
- **Path sanitization at IPC boundary**: All path strings must pass through `sanitize_display_path()` before crossing IPC. Both TOML-sourced and SQLite-sourced paths.
- **[NEW] `deleted_at IS NULL` filter**: Health queries joining `profiles` table must filter soft-deleted profiles. Use established pattern from `collections.rs` and `profile_sync.rs`.

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| ID  | Finding                                            | Risk                                                 | Mitigation                                                                                            |
| --- | -------------------------------------------------- | ---------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| W-1 | CSP disabled (`"csp": null` in `tauri.conf.json`)  | XSS could probe arbitrary paths via new IPC commands | Enable CSP: `"csp": "default-src 'self'; script-src 'self'"`. **More urgent in v2 with new commands** |
| W-2 | Raw paths in IPC responses (TOML + SQLite sources) | Leaks filesystem layout                              | Apply `sanitize_display_path()` to all path fields before IPC serialization                           |
| W-3 | Diagnostic bundle path leak (#49 downstream)       | Health reports exported in bundle expose filesystem  | Sanitize all paths before export                                                                      |
| N-1 | SQLite-sourced paths in filesystem ops             | Path traversal from corrupted DB                     | Re-validate paths read from SQLite before `metadata()` calls; defense-in-depth                        |
| N-3 | Sanitize before persistence, not only before IPC   | Unsanitized paths written to SQLite (Phase D)        | Apply `sanitize_display_path()` at struct-assembly time, covering both persistence and IPC            |
| N-4 | Health queries must filter `deleted_at IS NULL`    | Ghost profiles in health results                     | Build profile list from `ProfileStore::list()` (TOML-authoritative); filter SQLite joins              |

#### Advisories — Best Practices

- **A-1**: Distinguish ENOENT vs. EACCES via `std::fs::metadata()` error kinds (implement in Phase A)
- **A-2**: Symlink following via `metadata()` is correct; document in code
- **A-3**: TOCTOU is inherent — health check is advisory only; display "checked at" timestamp
- **A-5**: Batch validation sequential or bounded to 4 concurrent; profile if needed
- **N-2**: `diagnostic_json` free-text fields require `sanitize_diagnostic_report()` before IPC; prefer promoted `severity`/`failure_mode` columns

---

## Task Breakdown Preview

### Pre-Ship Security

**Focus**: Address security warnings before expanding IPC surface.
**Tasks**:

- Enable CSP in `tauri.conf.json` (W-1) — one-line change + testing
- Verify `sanitize_display_path()` is available in `src-tauri/src/commands/shared.rs` (W-2)

### Phase A: Core Health Check (MVP)

**Focus**: Batch validation, inline badges, remediation hints. **Zero MetadataStore code.**
**Tasks**:

1. Promote `require_directory()`, `require_executable_file()`, `is_executable_file()` to `pub(crate)` in `request.rs`
2. Create `HealthStatus`, `ProfileHealthReport`, `HealthIssue`, `HealthIssueSeverity` types in new `profile/health.rs`
3. Implement `check_profile_health()` — method-aware path validation on `GameProfile` fields directly
4. Implement `batch_check_health()` — iterate `ProfileStore::list()`/`load()`, catch per-profile errors as Broken
5. Write Rust unit tests using `tempfile::tempdir()` + `ProfileStore::with_base_path()` pattern
6. Add `batch_validate_profiles` and `get_profile_health` Tauri commands in new `commands/health.rs` (sanitize paths)
7. Create TypeScript types in `src/types/health.ts`
8. Create `useProfileHealth` hook (mirrors `useLaunchState` reducer pattern)
9. Create `HealthBadge` component (reuse `crosshook-status-chip` CSS)
10. Add inline health badges to profile selector list
11. Add per-issue remediation detail with `CollapsibleSection`
12. Hook into `save_profile` to auto-revalidate saved profile

**Parallelization**: Tasks 1-5 (Rust) can run in parallel with tasks 7-9 (TypeScript types + hook + component). Tasks 6 and 10-12 depend on both.

### Phase B: Metadata Enrichment

**Focus**: Leverage existing MetadataStore queries for richer health signals. **No new SQL, no new tables, no new migrations.**
**Dependencies**: Phase A complete.
**Tasks**:

- Enrich `batch_validate_profiles` with `query_failure_trends(30)` results
- Enrich with `query_last_success_per_profile()` timestamps
- Add launcher drift detection via `launchers.drift_state` query
- Add `ProfileHealthMetadata` enrichment type to Tauri command response
- Add failure trend badge overlay to `HealthBadge` component
- Add "last successful launch" label in detail panel
- Add launcher drift indicator
- Add collection/favorites filter controls to health view
- Add "Unconfigured" detection for brand-new all-empty profiles
- Add community-import context note (using `is_community_import` from metadata)

### Phase C: Startup Integration

**Focus**: Always-on background validation at startup.
**Dependencies**: Phase A complete.
**Tasks**:

- Spawn non-blocking async health check after UI renders
- Emit `profile-health-batch-complete` Tauri event
- Add startup summary banner for broken profiles (non-modal, dismissible)

### Phase D: Persistence + Trends

**Focus**: Health snapshot persistence for instant startup display and trend tracking.
**Dependencies**: Phase B complete.
**Tasks**:

- Add `health_snapshots` table (migration v6 in `migrations.rs`)
- Add `metadata/health_store.rs` with upsert/load/lookup functions
- Add `MetadataStore` methods for health snapshot persistence
- Persist health results on each batch validation
- Load cached snapshots on startup for instant badge display (before live scan completes)
- Add trend arrows in UI ("got better/worse since last check")
- Add stale-snapshot detection (>7 days old → prompt re-check)

---

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **Health Issue Type Reuse**
   - Options: (A) Reuse `LaunchValidationIssue` from `request.rs`, (B) New `HealthIssue` type in `health.rs`
   - Impact: Option A avoids type duplication; Option B adds a `field` discriminant for targeted UI
   - Recommendation: Practices-researcher favors Option A; if `field` tag is needed, add it to existing type

2. **Composite Health Display**
   - Options: (A) Separate indicators (filesystem badge + trend overlay + drift indicator), (B) Merged single badge with combined scoring
   - Impact: Option A is clearer but more visual elements; Option B risks confusing users
   - Recommendation: **Option A** — keep separate until user feedback validates merging

3. **Phase D Timing**
   - Options: (A) Include `health_snapshots` migration in Phase B for instant startup, (B) Defer to Phase D
   - Impact: Option A adds migration coupling to Phase B; Option B means no instant startup badges until later
   - Recommendation: **Option B** — Phase A/B should have zero migration dependency

---

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): SQLite query patterns, rusqlite APIs, Tokio/Tauri library analysis, health persistence forward spec (Phase D)
- [research-business.md](./research-business.md): Revised business rules (especially Rule 8 persistence), composite health signals, fail-soft boundaries, launcher drift rules
- [research-technical.md](./research-technical.md): Two-layer architecture design, complete Rust struct + TypeScript interface definitions, Tauri command contracts, health_snapshots schema
- [research-ux.md](./research-ux.md): Metadata-enriched health UX (trend badges, last-success labels, drift indicators), Steam Deck gamepad patterns, progressive disclosure
- [research-security.md](./research-security.md): 0 critical, 3 original warnings + 3 new findings (N-1 through N-4), SQLite3 spec cross-reference
- [research-practices.md](./research-practices.md): 18 reusable code items (11 original + 7 new from metadata layer), KISS verdict on SQLite integration, testability patterns with `MetadataStore::open_in_memory()`
- [research-recommendations.md](./research-recommendations.md): Revised 4-phase plan (A/B/C/D), 6 new quick wins from metadata layer, updated risk assessment, removed risks addressed by SQLite implementation
