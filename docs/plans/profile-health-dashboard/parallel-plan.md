# Profile Health Dashboard Implementation Plan

The profile health dashboard adds batch filesystem-path validation to CrossHook's profile management, surfacing per-profile health status (healthy/stale/broken) inline on the profile list. Implementation follows two parallel tracks — a Rust backend track (new `profile/health.rs` module with types, validation logic, and Tauri commands) and a TypeScript frontend track (types, hook, badge component) — converging at the `ProfilesPage.tsx` integration point. The critical path runs through `sanitize_display_path()` migration → Tauri commands → `useProfileHealth` hook → ProfilesPage integration, while all other tasks can be parallelized around this chain.

## Critically Relevant Files and Documentation

- docs/plans/profile-health-dashboard/feature-spec.md: Complete business rules, data model definitions (Rust structs + TypeScript interfaces), API contracts, UX workflows, security findings — the authoritative spec
- docs/plans/profile-health-dashboard/research-technical.md: Detailed validation logic pseudocode, `check_file_path`/`check_directory_path`/`check_executable_path` helper patterns, `batch_validate()` implementation, `derive_status()` logic
- docs/plans/profile-health-dashboard/research-business.md: Health classification rules, method-aware validation matrix, required vs optional field rules, notification behavior, remediation text guidance
- docs/plans/profile-health-dashboard/research-security.md: Security findings — 0 critical, 3 warnings (CSP, path sanitization, diagnostic bundle), 5 advisories
- docs/plans/profile-health-dashboard/research-practices.md: Reusable code inventory, module boundary rationale, KISS assessment, testability patterns
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: `validate_all()`, `ValidationError` enum with `.help()`, private path-checking helpers (`require_directory()`, `require_executable_file()`, `is_executable_file()`) to promote to `pub(crate)`
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` struct with all path fields; `resolve_launch_method()` for method-aware dispatch
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore::list()`, `load()`, `with_base_path()` — batch iteration and test harness
- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs: Module root — add `pub mod health;`
- src/crosshook-native/src-tauri/src/commands/profile.rs: Existing profile CRUD commands — add health commands here
- src/crosshook-native/src-tauri/src/commands/launch.rs: `sanitize_display_path()` at line ~301 — must move to `shared.rs`
- src/crosshook-native/src-tauri/src/commands/shared.rs: Already has `create_log_path()`, `slugify_target()` — destination for `sanitize_display_path()`
- src/crosshook-native/src-tauri/src/lib.rs: `invoke_handler!` command registration; async startup task spawn pattern at lines ~46-56
- src/crosshook-native/src-tauri/tauri.conf.json: `"csp": null` at line ~23 — security W-1
- src/crosshook-native/src/components/CompatibilityViewer.tsx: `crosshook-status-chip crosshook-compatibility-badge--{rating}` badge pattern
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: Expandable section for health detail panels
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Primary frontend integration point for inline health badges
- src/crosshook-native/src/hooks/useLaunchState.ts: `useReducer` + typed actions pattern — template for `useProfileHealth`
- src/crosshook-native/src/types/launch.ts: `LaunchValidationSeverity`, `LaunchFeedback` discriminated union — pattern for health types
- src/crosshook-native/src/styles/variables.css: CSS color tokens and touch target minimum
- src/crosshook-native/src/styles/theme.css: Where `crosshook-health-badge--{status}` CSS modifiers are added

## Implementation Plan

### Phase S: Security Pre-Ship

#### Task S.1: Enable Content Security Policy Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/tauri.conf.json
- docs/plans/profile-health-dashboard/research-security.md

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/tauri.conf.json

Enable CSP to close security warning W-1 before expanding the IPC surface. Change `"csp": null` (line ~23) to `"csp": "default-src 'self'; script-src 'self'"`. For development mode with Vite (`devUrl: "http://localhost:5173"`), the dev CSP may need `'unsafe-eval'` — test both dev and production build to ensure existing functionality works.

#### Task S.2: Move `sanitize_display_path()` to shared module Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs
- src/crosshook-native/src-tauri/src/commands/shared.rs

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/shared.rs
- src/crosshook-native/src-tauri/src/commands/launch.rs
- src/crosshook-native/src-tauri/src/commands/mod.rs

Move the `sanitize_display_path()` function from `commands/launch.rs` (line ~301) to `commands/shared.rs`. Make it `pub(crate)`. In `launch.rs`, replace the local definition with `use super::shared::sanitize_display_path;`. In `commands/mod.rs`, add `pub use shared::sanitize_display_path;` to re-export the function so that `lib.rs` (outside the `commands` module) can access it as `crate::commands::sanitize_display_path` — this is needed by the Phase C startup health scan in `lib.rs`. Verify `cargo build` succeeds and all existing callers in `launch.rs` still compile.

### Phase A: Core Health Check (MVP)

#### Task A.1: Promote path-checking helpers to `pub(crate)` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs

Change visibility of three private functions to `pub(crate)`:

- `fn require_directory(...)` → `pub(crate) fn require_directory(...)`
- `fn require_executable_file(...)` → `pub(crate) fn require_executable_file(...)`
- `fn is_executable_file(...)` → `pub(crate) fn is_executable_file(...)`

These are at approximately lines 698–756. Three one-line visibility keyword changes. Run `cargo check -p crosshook-core` to verify no compilation errors.

#### Task A.2: Create `profile/health.rs` with types and validation logic Depends on [A.1]

**READ THESE BEFORE TASK**

- docs/plans/profile-health-dashboard/feature-spec.md (§Data Models, §Business Requirements)
- docs/plans/profile-health-dashboard/research-technical.md (§Core Validation Logic)
- docs/plans/profile-health-dashboard/research-business.md (§Business Rules, §Path Issue Classification)
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/health.rs

Implement the core health validation module. This is the single largest new file.

**Types to define** (all with `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`):

- `ProfileHealthStatus` enum: `Healthy | Stale | Broken` (with `#[serde(rename_all = "snake_case")]`)
- `HealthIssueKind` enum: `NotConfigured | Missing | Inaccessible | WrongType` (with `#[serde(rename_all = "snake_case")]`)
- `ProfileHealthIssue` struct: `{ field: String, path: String, message: String, help: String, kind: HealthIssueKind }`
- `ProfileHealthResult` struct: `{ name: String, status: ProfileHealthStatus, launch_method: String, issues: Vec<ProfileHealthIssue>, checked_at: String }`
- `HealthCheckSummary` struct: `{ profiles: Vec<ProfileHealthResult>, healthy_count: usize, stale_count: usize, broken_count: usize, total_count: usize, validated_at: String }`

**Functions to implement**:

- `check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthResult`: Method-aware validation using `resolve_launch_method(profile)`. Check `game.executable_path` (all methods), `steam.compatdata_path` + `steam.proton_path` (steam_applaunch only), `runtime.prefix_path` + `runtime.proton_path` (proton_run only). Use `std::fs::metadata()` to distinguish `NotFound` from `PermissionDenied`. Derive status from worst issue kind: `Missing` → Stale, `NotConfigured`(required)/`Inaccessible`/`WrongType` → Broken.
- `batch_check_health(store: &ProfileStore) -> Result<HealthCheckSummary, ProfileStoreError>`: Iterate `store.list()`, `store.load()` per profile. Catch load errors per-profile as `Broken` entries — never propagate with `?` from inside the loop. Compute aggregate counts.
- Private helpers: `check_file_path()`, `check_directory_path()`, `check_executable_path()` — each takes field name, path string, severity, remediation text, and pushes to an issues vec.

**Key constraints**:

- Do NOT construct a `LaunchRequest` — validate `GameProfile` fields directly
- Use `std::fs::metadata()` not `Path::exists()` — returns `Result` that distinguishes error kinds
- `checked_at` uses `chrono::Utc::now().to_rfc3339()` (chrono is already a transitive dependency)
- Skip `trainer.path` filesystem validation — it's a WINE-mapped path, not a host path

#### Task A.3: Write unit tests for health module Depends on [A.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/health.rs
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs (test patterns)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/health.rs

Add a `#[cfg(test)] mod tests { ... }` section at the bottom of `health.rs`. Use `tempfile::tempdir()` + `ProfileStore::with_base_path(tmp.path().join("profiles"))` pattern from existing `toml_store.rs` tests. Create real fixture files on disk for "healthy" test cases. Test cases:

- Healthy profile (all required paths exist as correct types)
- Stale profile (configured path missing from disk → Missing → Stale)
- Broken profile (required field empty → NotConfigured → Broken)
- Mixed issues (both Missing and NotConfigured → Broken wins)
- Profile load error (corrupt TOML → Broken with load-error message)
- Method-aware: `steam_applaunch` profile skips `runtime.*` paths; `proton_run` skips `steam.*` paths
- Empty optional fields produce no issues
- `batch_check_health()` continues despite one profile failing to load

Run: `cargo test -p crosshook-core -- health`

#### Task A.4: Wire health module into profile mod Depends on [A.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs

Add `pub mod health;` to the module list. Add explicit named re-exports to match the existing style in `profile/mod.rs` (which uses named re-exports throughout, not glob `pub use *`): `pub use health::{ProfileHealthStatus, ProfileHealthResult, ProfileHealthIssue, HealthIssueKind, HealthCheckSummary, check_profile_health, batch_check_health};`. Verify with `cargo check -p crosshook-core`.

#### Task A.5: Add Tauri health check commands Depends on [A.2, A.4, S.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/src-tauri/src/commands/shared.rs
- src/crosshook-native/src-tauri/src/lib.rs

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/src-tauri/src/lib.rs

Add two new Tauri commands to `commands/profile.rs`:

1. `batch_validate_profiles(store: State<'_, ProfileStore>) -> Result<HealthCheckSummary, String>`: Call `crosshook_core::profile::health::batch_check_health(&store)`, then sanitize all path fields in each `ProfileHealthIssue` using `super::shared::sanitize_display_path()` before returning. Map errors via `.map_err(|e| e.to_string())`.

2. `get_profile_health(name: String, store: State<'_, ProfileStore>) -> Result<ProfileHealthResult, String>`: Load single profile, call `check_profile_health()`, sanitize paths, return.

Register both in `src-tauri/src/lib.rs` `invoke_handler!` macro by appending:

```
commands::profile::batch_validate_profiles,
commands::profile::get_profile_health,
```

#### Task A.6: Create TypeScript health types Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/profile-health-dashboard/feature-spec.md (§TypeScript Interfaces)
- src/crosshook-native/src/types/launch.ts

**Instructions**

Files to Create

- src/crosshook-native/src/types/health.ts

Files to Modify

- src/crosshook-native/src/types/index.ts

Create TypeScript interfaces mirroring Rust structs exactly (snake_case field names preserved across IPC):

- `ProfileHealthStatus = 'healthy' | 'stale' | 'broken'`
- `HealthIssueKind = 'not_configured' | 'missing' | 'inaccessible' | 'wrong_type'`
- `ProfileHealthIssue` interface: `{ field, path, message, help, kind }`
- `ProfileHealthResult` interface: `{ name, status, launch_method, issues, checked_at }`
- `HealthCheckSummary` interface: `{ profiles, healthy_count, stale_count, broken_count, total_count, validated_at }`

Add `export * from './health';` to `src/types/index.ts`.

#### Task A.7: Create `useProfileHealth` hook Depends on [A.5, A.6]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useLaunchState.ts
- src/crosshook-native/src/types/health.ts

**Instructions**

Files to Create

- src/crosshook-native/src/hooks/useProfileHealth.ts

Implement a `useReducer`-based hook following the `useLaunchState.ts` pattern:

**State**: `{ phase: 'idle' | 'loading' | 'loaded' | 'error'; summary: HealthCheckSummary | null; error: string | null }`

**Actions**: `{ type: 'check-start' } | { type: 'check-success'; summary: HealthCheckSummary } | { type: 'check-error'; error: string } | { type: 'single-update'; result: ProfileHealthResult }`

**Exposed API**:

- `summary: HealthCheckSummary | null` — current results
- `isLoading: boolean`
- `checkAll(): Promise<void>` — calls `invoke<HealthCheckSummary>('batch_validate_profiles')`
- `checkSingle(name: string): Promise<void>` — calls `invoke<ProfileHealthResult>('get_profile_health', { name })`, merges result into existing summary
- `getStatus(name: string): ProfileHealthStatus | undefined` — convenience lookup

The `single-update` action handler should: (1) replace the matching entry in `summary.profiles` by name, (2) recompute aggregate counts by iterating the updated profiles array and counting each status, (3) update `total_count`. Getting stale counts wrong would be a silent bug — always recompute from the full array, never increment/decrement.

#### Task A.8: Create `HealthBadge` component Depends on [A.6]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/CompatibilityViewer.tsx
- src/crosshook-native/src/styles/theme.css
- src/crosshook-native/src/styles/variables.css

**Instructions**

Files to Create

- src/crosshook-native/src/components/HealthBadge.tsx

Files to Modify

- src/crosshook-native/src/styles/theme.css

Create a presentational badge component following the `CompatibilityBadge` pattern:

```tsx
function HealthBadge({ status }: { status: ProfileHealthStatus }) {
  return (
    <span className={`crosshook-status-chip crosshook-health-badge crosshook-health-badge--${status}`}>
      {getHealthLabel(status)}
    </span>
  );
}
```

Map: `healthy` → "Healthy" + green, `stale` → "Stale" + amber, `broken` → "Broken" + red. Use existing CSS color tokens `--crosshook-color-success`, `--crosshook-color-warning`, `--crosshook-color-danger`.

Add CSS modifiers to `theme.css` (3 rules using existing color tokens):

```css
.crosshook-health-badge--healthy { ... }
.crosshook-health-badge--stale { ... }
.crosshook-health-badge--broken { ... }
```

Ensure `min-height: var(--crosshook-touch-target-min)` for gamepad accessibility.

#### Task A.9: Integrate health badges into ProfilesPage Depends on [A.7, A.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx
- docs/plans/profile-health-dashboard/research-ux.md (§User Workflows)
- docs/plans/profile-health-dashboard/research-business.md (§Workflows)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

This is the convergence task — wire everything together:

1. Import and use `useProfileHealth` hook. Call `checkAll()` on component mount.
2. Render `HealthBadge` adjacent to each profile name in the sidebar profile list. Pass `healthStatus` as a lookup from the summary.
3. Add a "Re-check All" button in the profile list header area. Wire to `checkAll()`.
4. Add per-profile health detail section using `CollapsibleSection` below the selected profile. Show each `ProfileHealthIssue` with field label, message, help text, and sanitized path.
5. Add a single "Open Profile" CTA per broken/stale profile that navigates to the profile editor by calling the existing profile selection dispatch from `useProfileContext()` (or equivalent) to set the selected profile name, which will load it in the editor area.
6. Wire auto-revalidation: after a successful `save_profile` invoke, call `checkSingle(profileName)` to update that profile's badge in-place.
7. Add community-import context note: when a profile has multiple `missing` issues, show "This profile was imported — paths may need to be updated for your system" before the issue list.
8. Show loading spinner badges while `checkAll()` is in progress; update atomically on completion.

Keep `ProfileFormSections.tsx` unchanged — inject health data at the `ProfilesPage.tsx` level only.

### Phase B: Polish

#### Task B.1: Enhance path error distinction and add optional path checks Depends on [A.9]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/health.rs
- docs/plans/profile-health-dashboard/research-security.md (§A-1)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/health.rs

1. Refine `std::fs::metadata()` error mapping: distinguish `io::ErrorKind::NotFound` (→ `Missing`) from `io::ErrorKind::PermissionDenied` (→ `Inaccessible`) with different remediation text ("file not found — re-browse" vs "file exists but permission denied — check ownership").
2. Add DLL injection path validation: iterate `profile.injection.dll_paths`, validate each non-empty entry as an existing file with `Warning` severity.
3. Add launcher icon path validation: check `profile.steam.launcher.icon_path` if non-empty with `Info` severity.
4. Add "Unconfigured" detection: when ALL path fields are empty, set status to `Broken` but add a synthetic issue with kind `NotConfigured` and help text "This profile has not been configured yet." The frontend can use this to soften the badge tone.
5. Update unit tests for new cases.

#### Task B.2: Add profile health filtering and sorting Depends on [A.9]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

Add a filter/sort control to the profile list that allows sorting by health status (broken first, then stale, then healthy). Implement as a toggle button or dropdown in the profile list header. Default sort remains alphabetical; health sort is opt-in.

### Phase C: Startup Integration

#### Task C.1: Add background startup health scan Depends on [A.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/lib.rs (lines ~46-56 for existing startup pattern)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

Spawn a non-blocking async health check after app startup, following the existing auto-load-profile pattern:

```rust
let handle = app.handle().clone();
tauri::async_runtime::spawn(async move {
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let store = handle.state::<ProfileStore>();
    match crosshook_core::profile::health::batch_check_health(&store) {
        Ok(summary) => {
            let _ = handle.emit("profile-health-batch-complete", &summary);
        }
        Err(e) => {
            tracing::warn!("Startup health check failed: {}", e);
        }
    }
});
```

Apply `sanitize_display_path()` to all path fields before emitting. Do NOT add to `startup.rs` — this goes in the `setup` closure in `lib.rs`.

#### Task C.2: Add startup health notification banner Depends on [A.9, C.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/hooks/useProfileHealth.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useProfileHealth.ts
- src/crosshook-native/src/components/pages/ProfilesPage.tsx

1. In `useProfileHealth.ts`: Add a `useEffect` that registers a Tauri event listener for `profile-health-batch-complete` using `listen<HealthCheckSummary>()` from `@tauri-apps/api/event`. When the event fires, dispatch `check-success` with the payload. Clean up the listener on unmount.

2. In `ProfilesPage.tsx`: When the summary arrives with `broken_count > 0`, show a dismissible startup banner: "N profiles have broken paths — click to review". Use the existing `crosshook-rename-toast` pattern with `role="status"` and `aria-live="polite"`. Banner persists until dismissed (per-session; re-shows next launch). Stale-only results show badge updates but no banner.

## Advice

- **`ValidationError::severity()` always returns `Fatal`** — the health module does NOT reuse `ValidationSeverity`. It uses `HealthIssueKind` for classification and derives `ProfileHealthStatus` from the worst kind. Do not call `.severity()` on `ValidationError` in health code.
- **`sanitize_display_path()` must be applied at the Tauri command layer**, not in `crosshook-core`. The core library should not depend on `$HOME` environment variable parsing — keep path sanitization as a display concern in the command layer. This means both `batch_validate_profiles` and `get_profile_health` commands must iterate through all `ProfileHealthIssue.path` fields and sanitize before returning.
- **Do NOT modify `ProfileFormSections.tsx`** — this 25k component renders the profile editor form. Health badges render in `ProfilesPage.tsx` sidebar list adjacent to profile names, outside the `ProfileFormSections` boundary. Threading health data through `ProfileFormSections` props would add risk for zero benefit.
- **`trainer.path` is a WINE-mapped path** — do not validate it against the host filesystem. It will always appear "missing" on the host because it's a path inside the WINE prefix. The existing `validate_all()` validates it at launch time within the Proton context.
- **`injection.dll_paths` is a `Vec<String>`** — iterate all entries, not just the first. Each non-empty DLL path needs its own health issue entry if missing.
- **The `is_stale: bool` on `LauncherInfo` and the `isStale()` function in `LaunchPanel.tsx` are both unrelated** to profile health staleness. `LauncherInfo.is_stale` is about exported launcher script freshness; `isStale()` is a 60-second preview age check. Do not reuse either.
- **Community-import context note is a Phase A must-have** (not Phase B) per team consensus — prevents users from blaming CrossHook when an imported profile has missing paths.
- **The `commands/shared.rs` module already exists** — it has `create_log_path()` and `slugify_target()`. S.2 moves `sanitize_display_path()` there; do not create a new file.
- **Auto-revalidation after save should use `checkSingle(name)` at the `ProfilesPage` level** — call it in the success path of `saveProfile`, not inside `useProfile.ts`. This keeps health concerns isolated from profile CRUD.
- **Phase B and Phase C are fully independent of each other** — they both depend only on Phase A being complete. They can run in parallel or in either order.
