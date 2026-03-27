# Profile Health Dashboard — Task Structure Analysis

The feature is well-scoped: one new Rust module, two new Tauri commands, one TypeScript types file, one hook, and one badge component — with integration into the existing profile list page. The Rust and TypeScript work streams are largely parallel. The critical path runs through `sanitize_display_path()` migration → Tauri commands → `useProfileHealth` hook → `ProfilesPage` integration.

---

## Executive Summary

- **Total new files**: 4 (health.rs, health.ts, useProfileHealth.ts, HealthBadge.tsx)
- **Total modified files**: 7 (request.rs, profile/mod.rs, commands/profile.rs, lib.rs, types/index.ts, ProfilesPage.tsx, commands/shared.rs)
- **Zero new dependencies** — all stdlib, already-present crates, existing CSS patterns
- **Phase A (MVP)** has two fully parallel tracks: Rust core (5 tasks) and TypeScript foundation (2 tasks), converging at Tauri commands + integration
- **Phases B and C** are both parallel after Phase A and independent of each other
- **Critical path**: S2 (move sanitize_display_path) → A5 (Tauri commands) → A7 (hook) → A9 (integration)

---

## Recommended Phase Structure

### Security Pre-Ship (run in parallel, before any Phase A work)

Both security tasks are atomic, independent, and affect subsequent work:

| Task                                | File(s)                                                   | Why first                                                                                                              |
| ----------------------------------- | --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| S1 — Enable CSP                     | `src-tauri/tauri.conf.json`                               | One-line change; security W-1; unblocks everything                                                                     |
| S2 — Move `sanitize_display_path()` | `src-tauri/src/commands/launch.rs` → `commands/shared.rs` | Path sanitization must be shared before health commands can use it; `shared.rs` already exists but lacks this function |

**Important**: `shared.rs` currently only has `create_log_path()` and `slugify_target()`. The `sanitize_display_path()` function lives in `commands/launch.rs` and must be moved to `shared.rs` + re-imported in `launch.rs`. This is the **only** pre-ship blocker for the health commands.

---

### Phase A — Core Health Check (MVP)

#### Track 1: Rust Core (sequential within track, parallel to Track 2)

| Task                                                    | Files                                                       | Blocks     | Notes                                                                                                                                                                                   |
| ------------------------------------------------------- | ----------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A1 — Promote path helpers                               | `crates/crosshook-core/src/launch/request.rs`               | A2         | Change `fn require_directory`, `fn require_executable_file`, `fn is_executable_file` to `pub(crate)`. 3 one-line visibility changes.                                                    |
| A2 — Create `profile/health.rs` with types + core logic | `crates/crosshook-core/src/profile/health.rs` (new)         | A3, A4, A5 | `ProfileHealthStatus`, `ProfileHealthIssue`, `HealthIssueKind`, `ProfileHealthResult`, `HealthCheckSummary`, `check_profile_health()`, `batch_check_health()`. Single largest new file. |
| A3 — Unit tests in `health.rs`                          | `crates/crosshook-core/src/profile/health.rs`               | —          | Inline tests using `tempfile::tempdir()` + `ProfileStore::with_base_path()`. Can be written alongside A2 or immediately after.                                                          |
| A4 — Wire module                                        | `crates/crosshook-core/src/profile/mod.rs`                  | A5         | Add `pub mod health;` + re-export types from `profile::health`. One-line change + pub use block.                                                                                        |
| A5 — Tauri commands                                     | `src-tauri/src/commands/profile.rs`, `src-tauri/src/lib.rs` | A7, A9     | Add `batch_validate_profiles` and `get_profile_health` commands; register in `invoke_handler!`. Depends on A2+A4 (types) and S2 (path sanitization via shared.rs).                      |

#### Track 2: TypeScript Foundation (parallel to Track 1)

| Task                         | Files                                             | Blocks | Notes                                                                                                                                                         |
| ---------------------------- | ------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A6 — TypeScript types        | `src/types/health.ts` (new), `src/types/index.ts` | A7, A8 | Pure type definitions; no Rust dependency; can start immediately. Mirror Rust structs exactly with `snake_case` field names.                                  |
| A8 — `HealthBadge` component | `src/components/HealthBadge.tsx` (new)            | A9     | Depends only on A6 types. Pure presentational component using `crosshook-status-chip crosshook-health-badge--{status}` CSS pattern from `CompatibilityBadge`. |

#### Integration (converges both tracks)

| Task                          | Files                                   | Blocks | Notes                                                                                                                                              |
| ----------------------------- | --------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| A7 — `useProfileHealth` hook  | `src/hooks/useProfileHealth.ts` (new)   | A9     | Depends on A5 (commands registered) and A6 (TS types). Mirror `useLaunchState.ts` `useReducer` pattern: `idle → loading → loaded \| error` states. |
| A9 — ProfilesPage integration | `src/components/pages/ProfilesPage.tsx` | —      | Depends on A7+A8. Wire health badges inline in profile selector. Wire `save_profile` → single-profile revalidation via `get_profile_health`.       |

---

### Phase B — Polish (after Phase A, internal tasks parallelizable)

| Task                                  | Files              | Notes                                                                               |
| ------------------------------------- | ------------------ | ----------------------------------------------------------------------------------- |
| B1 — ENOENT/EACCES distinction        | `health.rs`        | Refine `std::fs::metadata()` error kind mapping; security advisory A-1              |
| B2 — DLL + icon path checks           | `health.rs`        | Add `injection.dll_paths` (Vec iteration) and `steam.launcher.icon_path` validation |
| B3 — "Unconfigured" profile detection | `health.rs`        | Detect all-empty profile; soften to badge-only, no banner                           |
| B4 — Filter/sort by status            | `ProfilesPage.tsx` | Frontend-only; sort profile list by health status                                   |
| B5 — Community import context note    | `ProfilesPage.tsx` | Add contextual message for imported profiles with missing paths                     |

B1 + B2 + B3 can be done in a single `health.rs` commit. B4 + B5 are frontend-only and can be parallelized with B1-B3.

---

### Phase C — Startup Integration (after Phase A, parallel with Phase B)

| Task                                     | Files                                            | Notes                                                                                                                                                                                                                 |
| ---------------------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1 — Background startup scan             | `src-tauri/src/lib.rs`                           | Spawn async task: `sleep(500ms)` → run batch health check → `app_handle.emit("profile-health-batch-complete", summary)`. Follow existing `auto-load-profile` emit pattern (lines 46–56). Must NOT touch `startup.rs`. |
| C2 — Startup summary banner              | `src/components/pages/ProfilesPage.tsx`          | `listen("profile-health-batch-complete")` in `useProfileHealth`; show dismissible banner if `broken_count > 0`. Reuse `crosshook-rename-toast` pattern.                                                               |
| C3 — Extract `severityIcon()` (optional) | `src/utils/severity.ts` (new), `LaunchPanel.tsx` | Only needed if health dashboard reuses the same icon lookup; defer until Phase C integration reveals actual duplication.                                                                                              |

C1 can be done in parallel with C2 (C2 listens for the event C1 emits). C3 is optional and independent.

---

## Task Granularity Recommendations

### Keep Tasks Small and File-Focused

| Recommendation                                            | Rationale                                                                                          |
| --------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| A1 (visibility promotion) is its own task                 | Reviewed independently; unblocks A2; maps to a clean git commit                                    |
| A3 (tests) is tracked separately from A2 (implementation) | Tests can be written in parallel with implementation or reviewed as a separate pass                |
| A4 (module wiring) is separate from A2 (logic)            | One is a 3-line change; the other is 100+ lines — different risk profiles                          |
| S2 is a pre-ship task not bundled with A5                 | Refactoring an existing function before using it in new code is cleaner and independently testable |
| Phase B tasks B1+B2+B3 can be one batch                   | All in `health.rs`; same dev context; acceptable to combine since they're polish not MVP           |

### Tasks That Should NOT Be Split Further

| Task                           | Reason                                                                                                |
| ------------------------------ | ----------------------------------------------------------------------------------------------------- |
| A2 + A3 (can combine)          | Tests live inline in `health.rs` as a `#[cfg(test)]` module — same file                               |
| A5 (both commands in one task) | Two commands in the same file, registered together in `lib.rs`; splitting adds overhead without value |
| A6 (types + index.ts export)   | Trivially small; one-line barrel update ships with the types file                                     |

---

## Dependency Analysis

```
[S1] Enable CSP ──────────────────────────────────────────────► (no dependents)
[S2] Move sanitize_display_path ──────────────────────────────► A5

[A1] Promote pub(crate) helpers ──────────────────────────────► A2
[A2] Create profile/health.rs ─────────────────────────────┬──► A3, A4, A5
[A3] Write unit tests ─────────────────────────────────────┘   (no further dependents)
[A4] Wire profile/mod.rs ──────────────────────────────────────► A5
[A5] Tauri commands ──────────────────────────────────────────► A7, A9

[A6] TypeScript types + index.ts ─────────────────────────────► A7, A8
[A8] HealthBadge component ────────────────────────────────────► A9
[A7] useProfileHealth hook ────────────────────────────────────► A9
[A9] ProfilesPage integration ─────────────────────────────────► Phase B, Phase C

Phase B tasks (all parallel after A9) ──────────────────────────► (done)
Phase C tasks (C1 → C2, C3 independent) ────────────────────────► (done)
```

### Critical Path

```
S2 → A5 → A7 → A9
```

Total sequential chain: 4 tasks on the critical path. All other tasks can run in parallel around this chain.

### Maximum Parallelism Points

1. **S1 ∥ S2**: Both pre-ship tasks are independent
2. **A1 ∥ A6**: First Rust task and first TypeScript task are independent
3. **A2 ∥ A6 ∥ A8**: After A1 completes, A2 runs while TypeScript A6+A8 proceed
4. **A3 ∥ A4**: Tests and module wiring can proceed in parallel after A2
5. **Phase B ∥ Phase C**: Both polish phases are fully parallel after A9

---

## File-to-Task Mapping

### New Files

| File                                          | Task          | Track      |
| --------------------------------------------- | ------------- | ---------- |
| `crates/crosshook-core/src/profile/health.rs` | A2 + A3       | Rust       |
| `src/types/health.ts`                         | A6            | TypeScript |
| `src/hooks/useProfileHealth.ts`               | A7            | TypeScript |
| `src/components/HealthBadge.tsx`              | A8            | TypeScript |
| `src/utils/severity.ts`                       | C3 (optional) | TypeScript |

### Modified Files

| File                                          | Task(s)        | Risk                                                  |
| --------------------------------------------- | -------------- | ----------------------------------------------------- |
| `crates/crosshook-core/src/launch/request.rs` | A1             | Low — 3 visibility keyword changes                    |
| `crates/crosshook-core/src/profile/mod.rs`    | A4             | Low — add `pub mod health;` + re-export               |
| `src-tauri/src/commands/shared.rs`            | S2             | Low — move function, update 1 caller                  |
| `src-tauri/src/commands/launch.rs`            | S2             | Low — update import after S2 move                     |
| `src-tauri/src/commands/profile.rs`           | A5             | Medium — new command logic                            |
| `src-tauri/src/lib.rs`                        | A5, C1         | Low — append to macro list + spawn task               |
| `src/types/index.ts`                          | A6             | Low — one-line barrel add                             |
| `src/components/pages/ProfilesPage.tsx`       | A9, B4, B5, C2 | Medium — large existing component; inject health data |

---

## Optimization Opportunities

### 1. Type Alignment at IPC Boundary

The feature-spec defines `ProfileHealthResult.checked_at` as `String` (ISO 8601). Using `chrono::Utc::now().to_rfc3339()` in Rust and treating it as an opaque display string in TypeScript avoids time zone handling complexity. `chrono` is already a transitive dependency in the workspace.

### 2. `sanitize_display_path()` Move Improves Cohesion

Moving this function to `commands/shared.rs` during S2 reduces future IPC security risk holistically — any future command can import it from one place. The S2 diff to `launch.rs` will be: `pub(crate) use super::shared::sanitize_display_path;` replacing the local definition.

### 3. `ProfilesPage.tsx` Integration Strategy

The profile selector is rendered inside `ProfileFormSections` via props. Rather than threading health data through `ProfileFormSections` props (risk: large prop surface change), the recommended approach is:

- Pass a `healthStatus: Record<string, ProfileHealthStatus>` prop only to `ProfilesPage.tsx`
- Render `HealthBadge` adjacent to the profile name in the sidebar list (not inside `ProfileFormSections`)
- This keeps `ProfileFormSections.tsx` (25k component) unchanged and minimizes diff surface

### 4. Auto-Revalidate on Save

Feature-spec requires auto-revalidation after `save_profile`. Wire this in `useProfileHealth.ts` as a callback (`revalidateSingle(name: string)`) that calls `get_profile_health`. The `useProfile.ts` hook exposes `saveProfile` — call `revalidateSingle` in the success callback at the `ProfilesPage.tsx` level rather than modifying `useProfile.ts`, keeping health concerns separate.

### 5. Batch vs. Map Return Type for `batch_validate_profiles`

`analysis-code.md` suggests returning `HashMap<String, ProfileHealthSummary>` from `batch_validate_profiles`. The feature-spec defines `HealthCheckSummary` as a struct with a `profiles: Vec<ProfileHealthResult>` list. The struct form is preferred because it includes aggregate counts (`healthy_count`, `stale_count`, `broken_count`) in one IPC call, avoiding a second call to compute the summary. Keep the struct form from the feature-spec.

---

## Implementation Strategy Recommendations

### Start Sequence (Day 1)

1. **S1 + S2** in parallel — minimal change, security-correct baseline
2. **A1** — 5-minute visibility change; unlocks all Rust work
3. **A6** — TypeScript types from feature-spec data models; unlocks A8 immediately

### Parallel Development Window (Days 1–3)

After S1/S2/A1/A6 complete:

- **Developer A**: A2 (core logic) + A3 (tests) + A4 (module wiring)
- **Developer B**: A8 (`HealthBadge`) — can start after A6

### Convergence Point (Day 3–4)

After A2+A4+S2 are done:

- **A5**: Tauri commands — write, test manually, register in `lib.rs`
- Then **A7**: Hook wraps A5's commands
- Then **A9**: Integration in `ProfilesPage.tsx`

### Phase B + C (Days 4–5)

- B1+B2+B3 (all `health.rs`) in one session
- C1 (`lib.rs` spawn) + C2 (`ProfilesPage.tsx` banner listener) in one session
- B4+B5 (`ProfilesPage.tsx` polish) can be batched with C2 since they touch the same file

### Validation Gate Before Each Phase

| Gate          | Check                                                                           |
| ------------- | ------------------------------------------------------------------------------- |
| Before A5     | `cargo test -p crosshook-core` passes with new health.rs tests                  |
| Before A9     | Dev build: invoke `batch_validate_profiles` from frontend devtools manually     |
| After A9      | Smoke test: open app, verify health badges render on profile list               |
| After Phase C | Smoke test: startup banner appears for a profile with a missing executable path |

---

## Risk Flags for Implementors

| Risk                                                        | Mitigation                                                                                                                               |
| ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `ProfilesPage.tsx` already large (600+ lines)               | Inject health as a narrow prop — `healthStatus: Record<string, ProfileHealthStatus>` — rather than restructuring the component           |
| All `ValidationError::severity()` variants return `Fatal`   | Health module does NOT reuse `ValidationSeverity::Fatal`; uses `HealthIssueKind` for classification instead                              |
| `ProfileFormSections.tsx` renders the profile selector list | Do NOT modify this 25k file for health badges — render badges in `ProfilesPage.tsx` sidebar list next to profile names                   |
| Startup health check timing                                 | Use the existing pattern: `tauri::async_runtime::spawn` + `sleep(500ms)` + `app_handle.emit(...)` in `lib.rs`; never add to `startup.rs` |
| `injection.dll_paths` is a `Vec<String>`                    | Must iterate all entries; do not check only the first element                                                                            |
| IPC startup race                                            | Frontend calls `invoke('batch_validate_profiles')` on mount; do not rely on Rust push-only for startup results until Phase C             |
