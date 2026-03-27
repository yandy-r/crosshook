# Context Analysis: profile-health-dashboard

## Executive Summary

Profile Health Dashboard (GitHub #38, Phase 2) adds batch filesystem-path validation to CrossHook, surfacing per-profile health status (healthy/stale/broken) inline on the profile list. The feature builds on ~80% existing infrastructure with zero new Rust dependencies; the primary new work is a `profile/health.rs` module validating `GameProfile` fields directly via `std::fs::metadata()`, two Tauri IPC commands, a `useProfileHealth` hook, and a `HealthBadge` component reusing the existing `crosshook-status-chip` CSS pattern.

---

## Architecture Context

- **System Structure**: Three-tier — `crosshook-core/src/profile/health.rs` (domain logic) → `src-tauri/src/commands/profile.rs` (IPC commands) → `src/hooks/useProfileHealth.ts` + `src/components/HealthBadge.tsx` (React). Health is a profile-domain concern; no top-level `health/` module needed.
- **Data Flow**: `ProfileStore::list()` → per-profile `ProfileStore::load()` → `check_profile_health(GameProfile)` → `ProfileHealthResult` → Tauri IPC (sanitized) → `useProfileHealth` reducer → `HealthBadge` render. Startup scan: async `lib.rs` task emits `profile-health-batch-complete` event; frontend also calls `invoke('batch_validate_profiles')` on mount to handle race.
- **Integration Points**: `profile/mod.rs` gets `pub mod health;`; `commands/profile.rs` gets two new commands alongside existing CRUD; `src-tauri/src/lib.rs` gets command registration + optional startup async task; `ProfilesPage.tsx` gets inline `HealthBadge` per profile entry; `useProfile.ts` triggers single-profile revalidation after save.

---

## Critical Files Reference

- `crates/crosshook-core/src/launch/request.rs`: `require_directory()`, `require_executable_file()`, `is_executable_file()` (lines ~698–756) must be promoted to `pub(crate)` — health module shares these path-checking primitives
- `crates/crosshook-core/src/profile/models.rs`: All `GameProfile` path fields to validate (`game.executable_path`, `trainer.path`, `steam.proton_path`, `steam.compatdata_path`, `runtime.prefix_path`, `runtime.proton_path`, `injection.dll_paths`, `steam.launcher.icon_path`)
- `crates/crosshook-core/src/profile/toml_store.rs`: `list()`, `load()`, `with_base_path()` — batch iteration backbone and test harness pattern
- `crates/crosshook-core/src/profile/mod.rs`: One-line change: add `pub mod health;`
- `src-tauri/src/commands/profile.rs`: Home for `batch_validate_profiles` and `get_profile_health` commands; also contains `derive_steam_client_install_path()` which may need to move to `crosshook-core` (see Constraints)
- `src-tauri/src/commands/launch.rs`: `sanitize_display_path()` at line ~301 — must be moved to `commands/shared.rs` before Phase A (security W-2)
- `src-tauri/src/commands/shared.rs`: Already exists with `create_log_path()` and `slugify_target()` — `sanitize_display_path()` moves here; do not create a new file
- `src-tauri/src/lib.rs`: `invoke_handler!` command registration (line ~70); async startup task spawn pattern using `tauri::async_runtime::spawn` + `sleep(350ms)` + `app_handle.emit()` (lines ~46–56) — Phase C startup health scan mirrors this with `sleep(500ms)`
- `src-tauri/src/startup.rs`: **Do not touch** — synchronous path; health check must NOT be added here
- `src/components/CompatibilityViewer.tsx`: `crosshook-status-chip crosshook-compatibility-badge--{rating}` badge — copy pattern exactly for `HealthBadge`
- `src/components/ui/CollapsibleSection.tsx`: Already-built expandable section — reuse for health issue detail panels
- `src/hooks/useLaunchState.ts`: `useReducer` + typed actions pattern — template for `useProfileHealth` hook
- `src/types/launch.ts`: `LaunchValidationSeverity`, `LaunchFeedback` discriminated union — model for health TypeScript types
- `src/styles/variables.css`: `--crosshook-color-success/warning/danger`, `--crosshook-touch-target-min: 48px` — use these, add no new color tokens
- `src/components/pages/ProfilesPage.tsx`: Primary integration point — render `HealthBadge` adjacent to profile names in the **sidebar directly**, NOT threaded through `ProfileFormSections.tsx` (keep that 25k component unchanged)
- `src/components/ProfileFormSections.tsx`: **Do not modify** — renders profile selector via `profileSelector` prop; health badges sit outside this boundary in `ProfilesPage.tsx`

---

## Patterns to Follow

- **Tauri Command Pattern**: `#[tauri::command] pub fn cmd(state: State<ProfileStore>) -> Result<T, String>`. Frontend calls `invoke<T>('command_name', { args })`. See `commands/launch.rs::validate_launch`.
- **Async State Hook**: `useReducer` with discriminated action/state unions, `pending → loading → success | error` transitions. See `useLaunchState.ts:46`.
- **Status Badge**: `<span class="crosshook-status-chip crosshook-compatibility-badge--{rating}">` with CSS color tokens. Map `healthy→working`, `stale→partial`, `broken→broken`. See `CompatibilityViewer.tsx:76`.
- **Real-FS Testing**: `tempfile::tempdir()` + `ProfileStore::with_base_path(temp_path)`. No mocking. See `toml_store.rs` test functions.
- **Path Sanitization at IPC**: `sanitize_display_path()` on every path field before serialization across IPC. Non-negotiable (security W-2).
- **Per-Profile Error Isolation**: Catch `ProfileStoreError` per-profile in batch loop; emit as `Broken` entry. Never `?`-propagate from within the per-profile iteration.
- **Staleness Pattern**: `LauncherInfo.is_stale` in `launcher_store.rs:42` — the existing precedent for a staleness flag on stored data.

---

## Data Models (Locked — Do Not Deviate)

Full Rust structs defined in `feature-spec.md §Data Models`:

- `ProfileHealthStatus` enum: `Healthy | Stale | Broken`
- `HealthIssueKind` enum: `NotConfigured | Missing | Inaccessible | WrongType`
- `ProfileHealthIssue`: `{ field, path, message, help, kind }`
- `ProfileHealthResult`: `{ name, status, launch_method, issues, checked_at }`
- `HealthCheckSummary`: `{ profiles, healthy_count, stale_count, broken_count, total_count, validated_at }`

TypeScript interfaces mirror exactly (snake_case field names preserved across IPC). Create `src/types/health.ts`; add `export * from './health'` to `types/index.ts`.

---

## Cross-Cutting Concerns

- **Security pre-work is a hard blocker**: CSP enablement (`tauri.conf.json` line ~23) and `sanitize_display_path()` refactor to `commands/shared.rs` must land BEFORE Phase A tasks. Both affect the IPC surface this feature expands.
- **Method-aware validation required**: Only validate fields relevant to the profile's resolved launch method. `steam.proton_path` only for `steam_applaunch`; `runtime.prefix_path` only for `proton_run`. Empty optional fields produce no issue.
- **Severity precedence rule**: If both Stale and Broken issues exist, overall status is Broken. `Missing` → Stale; `NotConfigured` (required) / `Inaccessible` / `WrongType` → Broken.
- **`ValidationError::severity()` always returns `Fatal`** (confirmed at `request.rs:430`) — do not reuse it for health severity mapping. Health module derives status from `HealthIssueKind` directly using the precedence rule above.
- **Touch targets**: All interactive health elements need `min-height: 48px` (`--crosshook-touch-target-min`). Controller hints required: "Y Re-check" / "A Open" when broken profile focused.
- **No persistence**: Health results live in frontend state only — never written to disk. Invalidate on any profile save/rename/delete.

---

## Parallelization Opportunities

- **Phase A parallel track 1 (Rust backend)**: Promote `pub(crate)` helpers → implement `profile/health.rs` types and functions → write unit tests. Fully independent from TypeScript work.
- **Phase A parallel track 2 (TypeScript layer)**: Create `src/types/health.ts` → implement `useProfileHealth` hook → implement `HealthBadge` component. Can proceed in parallel with Rust once data model types are agreed.
- **Security pre-work**: CSP change (S1) and `sanitize_display_path()` refactor (S2) are independent of each other and can run in parallel.
- **Day 1 parallelism**: A1 (promote `pub(crate)` helpers in `request.rs`) and A6 (create `src/types/health.ts`) can start simultaneously with no dependencies between them.
- **Phase B and Phase C are independent of each other** — both depend only on Phase A being complete.
- **`src/utils/` directory does not yet exist** — if `severityIcon()` extraction (Phase C) proceeds, it creates this directory. Mark as optional; skip if not needed by health components.

---

## Implementation Constraints

- **Zero new Rust dependencies**: `std::fs`, `std::os::unix::fs::PermissionsExt`, `tokio` (already dep), `serde` (already dep), `tempfile` (already dev dep). Do not add `notify`, `rayon`, or `tokio::fs` for individual checks.
- **No `LaunchRequest` conversion path**: Feature-spec chose Option B — validate `GameProfile` fields directly. Do NOT implement `GameProfile::to_launch_request()`. (Note: `research-recommendations.md` argued for Option A with a `derive_steam_client_install_path()` move; feature-spec overrode this. If `steam_applaunch` validation requires `steam_client_install_path`, derive it from AppSettings in the health command, not from profile data.)
- **Do not reuse `isStale()` from `LaunchPanel.tsx`**: That function measures 60-second preview staleness — a completely different concept. Health staleness is filesystem-path-existence-based.
- **Startup path is synchronous**: `startup.rs` must not be modified. Spawn the health check from `lib.rs` as an async task after UI renders (mirrors existing startup task pattern at lines ~46–56).
- **No new Tauri capabilities required**: `std::fs::metadata()` in Rust-side commands does not need `fs:read` plugin.
- **Batch validation is synchronous** (`spawn_blocking` acceptable): 50 profiles × 8 paths ≈ 400ms worst case — acceptable for on-demand invoke. No `rayon`, no parallel futures.

---

## Key Recommendations

- **Write security pre-work as a blocking task before Phase A** — CSP and `sanitize_display_path()` refactor will cause rework if done after IPC commands are wired.
- **Phase A tasks 1–5 (Rust) and tasks 7–9 (TypeScript) are parallelizable** — separate agents can build the backend and frontend scaffolding simultaneously; integration tasks 6, 10–12 depend on both.
- **`ProfileHealthIssue` has a `field` string discriminant** — use this for targeted badge tooltips and remediation text. This is the key reason a new type was chosen over reusing `LaunchValidationIssue`.
- **Test first, integrate second**: Rust unit tests (task 5) should be completed before Tauri commands are wired (task 6) — the test harness validates the health logic independently and catches edge cases early.
- **Community-import context note** is a must-have for Phase A, not Phase B — prevents users from wrongly attributing broken state to CrossHook when they've imported an incompatible community profile.
- **Phase C startup integration is the smallest phase (0.5–1 day)** — defer it confidently; all badge and detail functionality is useful without it.
- **Critical path**: S2 (sanitize_display_path move) → Tauri commands (A5) → `useProfileHealth` hook (A7) → `ProfilesPage` integration (A9). Rust logic and TypeScript types are off the critical path and can parallelise with pre-work.

---

## Verified Codebase State (from code-analyzer + task-structurer)

| Claim                                                                    | Status                                                                              |
| ------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| `commands/shared.rs` already exists                                      | ✅ Confirmed — has `create_log_path()`, `slugify_target()`                          |
| `profile/mod.rs` has no health module                                    | ✅ Confirmed — exports: community_schema, exchange, legacy, models, toml_store only |
| `ValidationError::severity()` always returns `Fatal`                     | ✅ Confirmed at `request.rs:430`                                                    |
| `ProfileFormSections.tsx` renders profile selector                       | ✅ Confirmed — badges go in `ProfilesPage.tsx` sidebar, not through this component  |
| `lib.rs` uses `tauri::async_runtime::spawn` + `sleep(350ms)` for startup | ✅ Confirmed — Phase C uses same pattern with `sleep(500ms)`                        |
| `src/utils/` directory exists                                            | ❌ Does NOT exist — `severityIcon()` extraction would create it                     |
