# Proton Migration Tool — Recommendations & Risk Assessment

## Executive Summary

The Proton migration tool is a **low-to-medium complexity feature** with a **codebase readiness score of ~70%**. The existing infrastructure covers Proton discovery (`steam/proton.rs`), fuzzy version matching (`normalize_alias`, `resolve_compat_tool_by_name`), profile health validation (`profile/health.rs`), and batch profile iteration (`batch_check_health`). The primary new work is a **version suggestion function**, a **migration Tauri command with dry-run/confirm split**, and **Health Dashboard UI enhancements** to surface migration actions.

**Revised phasing recommendation:** Ship in **two phases** — Phase 1 delivers single-profile migration from the Health Dashboard to validate the algorithm and UX; Phase 2 adds batch migration with per-profile deselection. This aligns with business-analyzer and security-researcher findings that batch writes carry enough risk to warrant incremental rollout.

**No new crate dependencies required.** Zero external APIs involved. All needed capabilities exist in the current dependency tree.

### Cross-Team Synthesis

This document incorporates findings from all research teammates:

- **tech-designer**: Module placement, matching algorithm design, IPC architecture (separate command vs. health-embedded)
- **business-analyzer**: Version string parsing risk, cross-family false confidence, write atomicity, codebase readiness scoring
- **security-researcher**: 0 CRITICAL, 4 WARNING (W-1 through W-4), 7 ADVISORY (A-1 through A-7); non-atomic writes, local_override bypass, consent gate, partial failure
- **practices-researcher**: Reuse inventory (~7 major reuse points), module boundary recommendations, KISS assessment, zero new crates; escalated local_override targeting to design blocker and atomic writes to prerequisite
- **api-researcher**: Competitive analysis of Lutris/Heroic/Bottles/ProtonUp-Qt — **no existing Linux launcher implements proactive Proton migration**; this is a genuine differentiator. Rejected silent-fallback pattern (Lutris); recommended explicit confirmation model
- **ux-researcher**: Competitive UX patterns; inline auto-suggestion (VS Code pattern); before/after confirmation modal (NN/G); descriptive button labels ("Update 4 Profiles" not "Apply"); single-profile fix = undo toast, batch fix = modal; auto re-check after migration

---

## Implementation Recommendations

### Implementation Prerequisites (Must Address Before Phase 1)

These are **design blockers** escalated by practices-researcher and security-researcher. They are not optional enhancements — Phase 1 cannot ship correctly without them.

**Prerequisite 1: Crash-Safe Writes for Migration Path**

`ProfileStore::save()` uses `fs::write()` (truncate-then-write). If the process crashes or is killed mid-write, the profile TOML is corrupted. For migration — which modifies profiles the user did not manually open — this risk is elevated above normal save operations.

**Required fix:** The migration write path must use write-to-temp + `fs::rename()`:

```rust
// In profile/migration.rs apply logic:
let tmp_path = path.with_extension("toml.tmp");
fs::write(&tmp_path, toml::to_string_pretty(&storage_profile)?)?;
fs::rename(&tmp_path, &path)?; // POSIX rename(2) is atomic on same FS
```

This does NOT require changing `ProfileStore::save()` universally — only the migration write path. Both source and target are in `~/.config/crosshook/profiles/` (same filesystem), so `rename(2)` atomicity is guaranteed.

**~~Prerequisite 2~~ (Resolved — Not a Blocker): local_override Targeting**

Initially flagged as a design blocker, but confirmed safe by tech-designer and practices-researcher. The standard `load()` → mutate → `save()` pattern works correctly because `save()` calls `storage_profile()` (models.rs:272-291) which converts the effective form back to the correct on-disk form with machine-local paths in `local_override`. The `storage_profile_roundtrip_is_idempotent` test already validates this invariant.

A migration-specific round-trip test is still recommended for confidence, but this is not a prerequisite blocker.

**Prerequisite 2: Promote Private Functions to `pub(crate)`**

`normalize_alias()` (steam/proton.rs:411) and `resolve_compat_tool_by_name()` (steam/proton.rs:273) are private. Migration module needs both. ~5 line visibility change, zero behavior change.

---

### Recommended Approach: Separate Migration Module + Health Dashboard Integration

Place migration logic in a **new `profile/migration.rs` module** (tech-designer recommendation) with a **separate IPC command** for migration suggestions, keeping health checks fast and focused. Surface migration actions in the Health Dashboard UI.

**Why this approach (refined from team input):**

- `check_profile_health()` already detects missing proton paths (lines 376-396 of `health.rs`) — stale detection is free
- **Separate IPC for suggestions** (tech-designer Decision 5, Option B): health checks stay fast; migration discovery is queried on-demand when user views stale profiles
- New `profile/migration.rs` keeps migration logic distinct from discovery (`steam/proton.rs`) and health (`profile/health.rs`), matching the crate's existing module-per-concern structure
- **Dry-run/confirm split** (security-researcher W-3): preview returns `MigrationPlan` with no writes; apply only on explicit user confirmation
- No new page, no new navigation — adds "Fix" capability to existing Health Dashboard

**Architecture:**

```
profile/migration.rs (NEW)
  └─ suggest_proton_replacement(stale_path, installed_tools) → MigrationSuggestion
  └─ extract_proton_family(name) → family prefix string
  └─ plan_migration(profiles, installed_tools) → MigrationPlan (dry run)
  └─ apply_migration(plan, store) → Vec<MigrationResult>

steam/proton.rs (MODIFIED — visibility only)
  └─ promote normalize_alias() to pub(crate)
  └─ promote resolve_compat_tool_by_name() to pub(crate)

commands/migration.rs (NEW)
  └─ preview_proton_migration → MigrationPlan (no writes)
  └─ apply_proton_migration(plan, confirm: true) → Vec<MigrationResult>

Health Dashboard UI:
  └─ Stale Proton issues show "Suggest Fix" → queries preview_proton_migration
  └─ User sees before/after paths → clicks "Apply" → calls apply_proton_migration
  └─ Phase 2: Batch "Fix All" button with per-profile deselection
```

**Write safety (security-researcher W-1):** Migration writes use write-to-temp + `fs::rename()` pattern instead of direct `fs::write()` for crash-safe batch operations.

### Quick Wins

1. **Improve remediation text immediately** — Update the `remediation` string in `check_required_executable` (health.rs line 282) from generic "Re-browse to the executable" to: "Proton version not found. Check for updated versions in the Proton Path dropdown."

2. **Promote `normalize_alias` to `pub(crate)`** — Currently private in `steam/proton.rs` (line 411). Migration module needs it. ~5 line change, enables the entire suggestion engine.

3. **Add `AppMigration` variant to `SyncSource`** — In `metadata/profile_sync.rs`, add the enum variant so metadata correctly tracks migration-triggered writes. ~3 lines.

### Phasing Assessment (Revised)

**Two phases recommended** (aligning with business-analyzer and security-researcher findings):

**Phase 1 — Single-Profile Migration (validates algorithm + UX)**

1. Backend: Version suggestion engine in `profile/migration.rs`
2. Backend: Single-profile `preview_proton_migration` + `apply_proton_migration` Tauri commands
3. Frontend: Per-issue "Suggest Fix" / "Apply" in Health Dashboard rows
4. Crash-safe writes via temp file + rename

**Phase 2 — Batch Migration (scales the validated approach)**

1. Backend: Batch migration with pre-flight validation pass
2. Frontend: "Fix All Stale Proton Paths" toolbar button with deselection UI
3. Post-migration launcher re-export prompt
4. Optional: migration audit log in SQLite

---

## Improvement Ideas

### Competitive Differentiation (from api-researcher)

**No existing Linux game launcher implements proactive Proton migration.** Current ecosystem patterns:

- **Lutris**: Silent fallback to default Wine version — breaks trust, causes hard-to-diagnose issues. **Rejected for CrossHook.**
- **Heroic/Bottles**: Hard fail + generic error message — the current CrossHook status quo ("The Steam Proton path does not exist").
- **ProtonUp-Qt**: Manages installations but has no cross-reference to launcher profiles — creates the exact gap this feature fills.
- **Steam**: Per-game compat tool selection works only for `steam_applaunch`; no concept of `proton_run` profile migration.

CrossHook's proactive detection + guided migration is a **genuine UX differentiator** with no ecosystem precedent.

### Related Features

1. **Proton path auto-heal on app startup** — During the startup `batch_validate_profiles` event, populate migration suggestions. Users see "3 profiles have outdated Proton paths" toast on launch with a link to the Health Dashboard. (Warning-only; no auto-writes per W-3.)

2. **Profile-save-time inline suggestion** — When editing a profile with a stale Proton path, show inline auto-suggestion in `ProtonPathField` (VS Code npm-outdated pattern from ux-researcher). `ProtonPathField` already has error display and install dropdown. Additive to Phase 1 without conflict.

3. **Proton version pinning** — Optional `proton_version_pinned` metadata flag to suppress migration suggestions. Prevents nagging users who intentionally use older versions. Phase 2 enhancement. (Note: Heroic v2.18.0 learned the hard way that restricting version choices causes user backlash — pinning must be opt-in, never enforced.)

4. **Cross-major version prefix warning** — When suggested version is a different major version (e.g., 8.x → 9.x), display explicit warning about potential WINE prefix incompatibility. Steam itself shows this warning. (api-researcher)

5. **Migration history in metadata** — Store migration events in SQLite (`migration_events` table, modeled after existing `profile_name_history`). Enables audit trail and health trend tracking. Phase 2 (A-3).

6. **Launcher re-export after migration** — Detect affected exported launchers via existing `launcher_drift_map` and prompt re-export. Phase 2.

### UX Patterns to Adopt (from ux-researcher)

| Pattern                                | Source                            | Application                                                                                   |
| -------------------------------------- | --------------------------------- | --------------------------------------------------------------------------------------------- |
| **Inline auto-suggestion**             | VS Code npm-outdated              | Show suggestion directly in Health Dashboard issue row; zero extra navigation                 |
| **Before/after review modal**          | NN/G confirmation dialog research | Batch migration: profile names, old path → new path, per-row checkboxes                       |
| **Descriptive confirm buttons**        | NN/G guidelines                   | "Update 4 Profiles" not generic "Apply" / "OK"                                                |
| **Empathetic error messages**          | UX writing best practice          | "GE-Proton 9-4 is no longer installed" not "Error: path invalid"                              |
| **Single = undo toast, Batch = modal** | Dialog fatigue research           | Avoid modal for single-profile fix; use dismissible toast with undo. Require modal for batch. |
| **Auto re-check after migration**      | Immediate feedback loops          | Trigger `revalidateSingle()` / `batchValidate()` automatically; no manual refresh button      |
| **No optimistic UI updates**           | Data integrity                    | Wait for filesystem write confirmation before updating React state                            |

### Optimization Opportunities

1. **Cache Proton discovery results** — `discover_compat_tools()` scans the filesystem on every call. Cache results for the duration of a migration session rather than re-discovering per profile.

2. **Version family extraction** — Build `extract_proton_family()` that strips trailing digits from normalized alias. Enables same-family suggestions ranked by integer-tuple version comparison.

---

## Risk Assessment

### Security Findings Summary (from security-researcher)

**No CRITICAL findings.** Feature is local-only, filesystem-bounded, user-owned directories throughout. Final tally: 0 CRITICAL, 4 WARNING (W-2 resolved), 10 ADVISORY.

| ID   | Severity           | Finding                                                                                                                         | Required Action                                                                                                                                                                                                                                                                            |
| ---- | ------------------ | ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| W-1  | WARNING            | `ProfileStore::save()` uses `fs::write()` (truncate-then-write) — not crash-safe for migration                                  | Write to `.toml.tmp` then `fs::rename()` in migration write path. POSIX `rename(2)` is atomic within same filesystem. **Elevated to prerequisite.**                                                                                                                                        |
| W-2  | WARNING (resolved) | `steam.proton_path` is EMPTY in stored v0.2.4+ profiles; real value is in `local_override.steam.proton_path`.                   | Standard `load()` → modify effective → `save()` round-trips through `storage_profile()` correctly. Confirmed safe by tech-designer; `storage_profile_roundtrip_is_idempotent` test validates the invariant. **No longer a blocker.** Migration-specific round-trip test still recommended. |
| W-3  | WARNING            | No user consent gate at command layer                                                                                           | Dry-run/confirm split: preview returns `MigrationPlan` with no writes; apply only on explicit user confirmation. Must not auto-write from startup path. **No silent fallback** (validated by api-researcher competitive analysis — Lutris's silent fallback pattern causes trust issues).  |
| W-4  | WARNING            | Batch migration partial failure with no rollback                                                                                | Pre-flight validation pass (serialize all profiles + verify replacement paths) before any writes. Best-effort all-or-nothing for most failure modes.                                                                                                                                       |
| A-1  | ADVISORY           | Add `is_executable()` check (mode `& 0o111`) on replacement candidates                                                          | Deferrable — `discover_compat_tools()` already verifies `proton` file exists                                                                                                                                                                                                               |
| A-2  | ADVISORY           | Canonicalize paths before dedup to avoid symlink duplicates                                                                     | Deferrable                                                                                                                                                                                                                                                                                 |
| A-3  | ADVISORY           | SQLite migration_log table for audit trail (model: existing `profile_name_history` table)                                       | Phase 2                                                                                                                                                                                                                                                                                    |
| A-4  | ADVISORY           | Apply `sanitize_display_path()` to all paths in `MigrationPlan`/`MigrationResult` before IPC return                             | **Phase 1 — mandatory.** Matches existing pattern in `commands/health.rs` and `commands/launch.rs`.                                                                                                                                                                                        |
| A-5  | ADVISORY           | Validate `steam_client_install_path` IPC argument with `candidate.join("steamapps").is_dir()` check                             | Low risk but cheap validation — include in Phase 1.                                                                                                                                                                                                                                        |
| A-6  | ADVISORY           | Profile TOML files written with process umask (0o644) while SQLite uses 0o600 — inconsistency                                   | Not migration-specific; can be improved separately.                                                                                                                                                                                                                                        |
| A-7  | ADVISORY           | No file locking between user saves and batch migration — last-writer-wins race                                                  | Consistent with existing `save_launch_optimizations` pattern. Acceptable risk.                                                                                                                                                                                                             |
| A-8  | ADVISORY           | Staleness check must use `try_exists()` not `exists()` — `exists()` swallows permission errors → false-positive stale detection | Use `stored_path.try_exists().map(\|e\| !e).unwrap_or(false)`. Do NOT canonicalize stored path before check (users may store symlinked form).                                                                                                                                              |
| A-9  | ADVISORY           | File picker ("Browse for Proton") results bypass `discover_compat_tools()` validation                                           | Validate user-selected paths with `is_file() && permissions().mode() & 0o111 != 0` — same pattern as `update/service.rs:137`.                                                                                                                                                              |
| A-10 | ADVISORY           | XSS-equivalent concern for path display in Tauri WebView                                                                        | Confirmed safe: zero `dangerouslySetInnerHTML` in codebase. React JSX auto-escapes. Use `{path}` in JSX, never raw HTML APIs.                                                                                                                                                              |

**Data integrity requirement:** After successful migration, invalidate `health_snapshots` rows for migrated profile IDs. Otherwise the Health Dashboard continues showing stale-path issues for profiles that were just fixed. Frontend must trigger `revalidateSingle()` or `batchValidate()` post-migration.

### Technical Risks

| Risk                                                                                                                  | Severity          | Likelihood | Mitigation                                                                                                                                                                                                                                               |
| --------------------------------------------------------------------------------------------------------------------- | ----------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Version matching suggests wrong family** (e.g., GE-Proton → Proton Experimental)                                    | Medium            | Medium     | Extract family prefix by removing trailing digits from normalized name. Only suggest within same family; cross-family matches excluded from batch operations and shown with low confidence. (business-analyzer Risk 2)                                   |
| **Version string mis-ordering** — "9-10" sorts before "9-9" lexicographically                                         | Medium            | High       | Parse dash/dot-separated numeric segments and compare as integer tuples, not string sort. (business-analyzer Risk 1)                                                                                                                                     |
| **Cross-major Proton upgrade corrupts WINE prefix** — migrating from Proton 8.x to 9.x can break prefix compatibility | Medium            | Medium     | Display explicit warning when suggested version is a different major version. api-researcher confirms Steam itself warns about this. Do not auto-select cross-major upgrades in batch operations.                                                        |
| ~~**Local override layer bypassed**~~                                                                                 | ~~High~~ Resolved | ~~Medium~~ | W-2 resolved. Standard `load()` → modify → `save()` pattern correctly round-trips through `storage_profile()`. Confirmed by tech-designer; existing `storage_profile_roundtrip_is_idempotent` test validates. Migration-specific test still recommended. |
| **Concurrent save race** — user saves while migration runs                                                            | Low               | Low        | Write-to-temp + `fs::rename()` makes writes atomic. Last write wins — user-initiated save should take precedence. Consistent with existing `save_launch_optimizations` pattern (A-7).                                                                    |
| **Steam root discovery for runtime.proton_path users** — no `compatdata_path` to derive steam root                    | Medium            | Low        | Fall back to scanning default Steam root paths (`~/.local/share/Steam`, `~/.steam/root`, Flatpak `~/.var/app/com.valvesoftware.Steam/data/Steam`). (business-analyzer Risk 5)                                                                            |
| **Profile with both steam.proton_path and runtime.proton_path**                                                       | Medium            | Low        | Only migrate the field matching `resolve_launch_method()`.                                                                                                                                                                                               |
| **Exotic Proton naming conventions** (TKG, SteamTinker builds) mis-ranked by digit extraction                         | Low               | Low        | Require explicit user confirmation before any change. Phase 1 single-profile confirmation mitigates this. (api-researcher)                                                                                                                               |

### Integration Challenges

1. **Separate IPC for suggestions** (tech-designer Decision 5) — Migration suggestions are fetched via dedicated `preview_proton_migration` command, not embedded in health check results. Health check stays fast; frontend queries migration preview on-demand when user expands a stale profile's issues.

2. **ProtonPathField dropdown sync** — After migration, the `ProtonPathField` dropdown should reflect the new selection. The `installs` prop is populated by `list_proton_installs` Tauri command on mount — no sync issue since migration uses the same discovered list.

3. **Metadata health snapshot update** — After migration, trigger `revalidateSingle(profileName)` from frontend to refresh the snapshot. Also call `observe_profile_write()` with new `SyncSource::AppMigration` variant for metadata tracking.

4. **Launcher drift cascade** — Migrated profiles with exported launchers will have stale `.sh` scripts. Existing `launcher_drift_map` in health enrichment already detects this. Post-migration UI should surface "Launcher needs re-export" for affected profiles.

5. **`normalize_alias` visibility** — Currently private in `steam/proton.rs`. Must be promoted to `pub(crate)` before migration module can reuse it. (~5 line change, noted by practices-researcher.)

### Performance Concerns

- **Batch migration with many profiles** — `discover_compat_tools()` scans filesystem directories (~50-100ms for 5+ library folders, 20+ Proton versions). Call once, pass result to all migration checks. Per-profile migration is just string comparison — ~0ms additional.

- **Re-validation after batch migration** — `batch_validate_profiles` re-scans all profiles (1-2 seconds for 50+ profiles). Acceptable UX with existing loading indicator in Health Dashboard toolbar.

---

## Alternative Approaches

### Option A: Separate Migration Module + Health Dashboard Surface (Recommended)

**Description:** New `profile/migration.rs` module with dedicated `commands/migration.rs` Tauri commands (preview + apply). Health Dashboard surfaces stale Proton issues with "Suggest Fix" → preview → "Apply" flow. Suggestions fetched via separate IPC, not embedded in health check.

| Aspect         | Assessment                                                                                                                            |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**       | Clean module separation; health checks stay fast; dry-run/confirm consent gate; crash-safe writes; no new pages                       |
| **Cons**       | Two IPC calls per migration (preview + apply); slightly more wiring than embedding in health                                          |
| **Effort**     | Low-Medium (~4-5 files backend, ~2-3 files frontend)                                                                                  |
| **Code Reuse** | Very high — reuses `discover_compat_tools`, `normalize_alias`, `resolve_compat_tool_by_name`, `ProfileStore`, `observe_profile_write` |

### Option B: Standalone Migration Page

**Description:** New `MigrationPage.tsx` accessible from sidebar. Dedicated scan, preview, and apply workflow.

| Aspect         | Assessment                                                                                                          |
| -------------- | ------------------------------------------------------------------------------------------------------------------- |
| **Pros**       | Full control over UX; dedicated migration workflow; can include advanced options (version pinning, rollback)        |
| **Cons**       | Duplicates health check discovery; new sidebar entry on an already full nav (7 items); higher implementation effort |
| **Effort**     | Medium-High (~5-8 files backend, ~5-8 files frontend)                                                               |
| **Code Reuse** | Moderate — reuses discovery but duplicates profile iteration and UI patterns                                        |

### Option C: Profile-Save-Time Reactive (Additive Enhancement)

**Description:** When saving a profile with a stale Proton path, show inline suggestion in `ProtonPathField`. No batch capability.

| Aspect         | Assessment                                                                                                              |
| -------------- | ----------------------------------------------------------------------------------------------------------------------- |
| **Pros**       | Minimal implementation; natural UX (fix when editing); `ProtonPathField` already has error display and install dropdown |
| **Cons**       | No batch migration; user must open each profile individually; doesn't proactively surface issues                        |
| **Effort**     | Low (~1-2 files backend, ~1 file frontend)                                                                              |
| **Code Reuse** | Very high — uses ProtonPathField existing UI patterns                                                                   |

### Key Architectural Decisions (from tech-designer)

| Decision                       | Recommendation                                                                           | Rationale                                                                                                                                             |
| ------------------------------ | ---------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Migration logic placement**  | New `profile/migration.rs`, not extending `steam/proton.rs`                              | Migration is distinct from discovery; separate module keeps concerns clean while importing discovery functions                                        |
| **Family detection algorithm** | Strip trailing digits from normalized name; "geproton94" → family "geproton"             | Simple, handles all known Proton naming conventions. Edge cases: "Proton-Experimental" vs "Proton 9.0" are correctly identified as different families |
| **Batch atomicity**            | Best-effort with per-profile error reporting                                             | Matches existing `batch_check_health` pattern; Phase 2 adds pre-flight validation pass                                                                |
| **Suggestion confidence**      | Exact family + newer = High (0.9), Same family + older = Medium (0.7), Fuzzy = Low (0.5) | Cross-family suggestions (Low confidence) excluded from batch operations                                                                              |
| **Suggestion IPC**             | Separate command, not embedded in health                                                 | Health checks stay fast (~0ms overhead); migration discovery queried on-demand                                                                        |

### Recommendation

**Option A (Separate Migration Module + Health Dashboard)** as the primary approach, with **Option C (Profile-Save-Time Reactive)** as an additive follow-up enhancement. The Health Dashboard is the natural home for "things wrong with your profiles." Option C adds inline fix capability in `ProtonPathField` without conflicting with Option A.

---

## Task Breakdown Preview

### Phase 1 — Single-Profile Migration (validates algorithm + UX)

**Group 1.0: Prerequisites** (no dependencies — must complete first)

- [ ] Promote `normalize_alias()` to `pub(crate)` in `steam/proton.rs` (~1 line)
- [ ] Promote `resolve_compat_tool_by_name()` to `pub(crate)` in `steam/proton.rs` (~1 line)
- [ ] Add `AppMigration` variant to `SyncSource` enum in `metadata/profile_sync.rs` (~3 lines)
- [ ] (Recommended) Add migration-specific round-trip test for proton path through `load()` → modify → `save()` — existing `storage_profile_roundtrip_is_idempotent` test covers the invariant but a migration-focused test adds confidence
- **Estimated complexity:** Trivial — ~10 lines + 1 recommended test

**Group 1.1: Backend — Version Suggestion Engine** (depends on 1.0)

- [ ] Create `profile/migration.rs` module with `extract_proton_family(name: &str) -> Option<String>` — strips trailing digits from normalized alias to extract family prefix
- [ ] Add `suggest_proton_replacement(stale_path: &str, installed_tools: &[ProtonInstall]) -> Option<ProtonMigrationSuggestion>` — same-family match with integer-tuple version comparison
- [ ] Add `ProtonMigrationSuggestion` struct: `old_path`, `new_path`, `new_name`, `confidence` enum (ExactFamily/SameFamily/FuzzyMatch), `is_cross_major: bool`
- [ ] Version parsing: split on `-`/`.`, parse segments as integers, compare as tuples (handles "9-10" > "9-4" correctly)
- [ ] Flag cross-major suggestions with `is_cross_major = true` (prefix incompatibility warning)
- [ ] Unit tests: family extraction, same-family newer/older, cross-family exclusion, non-semver edge cases, no-match, cross-major detection, exotic naming (TKG/SteamTinker)
- **Estimated complexity:** Low — ~150 lines of new Rust code including tests

**Group 1.2: Backend — Migration Tauri Commands** (depends on 1.1)

- [ ] Create `commands/migration.rs` with `preview_proton_migration(profile_name)` → `MigrationPlan` (dry run, no writes)
- [ ] Add `apply_proton_migration(profile_name, new_proton_path)` → `MigrationResult`
- [ ] **Crash-safe writes:** write to `.toml.tmp` then `fs::rename()` (Prerequisite 1)
- [ ] Use `resolve_launch_method()` to target correct field (`steam.proton_path` vs `runtime.proton_path`)
- [ ] Load via `store.load()` → modify effective profile → save via `store.save()` (handles `storage_profile()` conversion — Prerequisite 2)
- [ ] Call `observe_profile_write()` with `SyncSource::AppMigration`
- [ ] Apply `sanitize_display_path()` to all paths in `MigrationPlan`/`MigrationResult` before IPC return (A-4 — mandatory)
- [ ] Re-validate replacement path existence immediately before applying (TOCTOU mitigation)
- [ ] After successful migration, invalidate `health_snapshots` row for migrated profile ID (data integrity)
- [ ] Unit tests: local_override round-trip correctness, method-specific field targeting, write atomicity, stale replacement rejection, path sanitization
- **Estimated complexity:** Medium — ~200 lines new Rust + ~50 lines Tauri wiring

**Group 1.3: Frontend — Single-Profile Migration UX** (depends on 1.2)

- [ ] Add `MigrationSuggestion` and `MigrationResult` TypeScript types
- [ ] In Health Dashboard issue rows for `missing_proton` category: add inline "Suggest Fix" button (VS Code npm-outdated pattern)
- [ ] On click: call `preview_proton_migration` → show before/after with confidence indicator
- [ ] Cross-major suggestions: display explicit prefix incompatibility warning
- [ ] "Update to [version name]" button (descriptive label, not generic "Apply") calls `apply_proton_migration`
- [ ] On success: show dismissible undo toast (not modal — avoid dialog fatigue for single-profile fix)
- [ ] Auto-trigger `revalidateSingle(profileName)` on success (no manual refresh needed)
- [ ] No optimistic UI updates — wait for filesystem confirmation before updating React state
- [ ] Empathetic error messages: "GE-Proton 9-4 is no longer installed" not "Error: path invalid"
- [ ] Sanitize displayed paths (home → `~`) using existing pattern
- **Estimated complexity:** Medium — ~180 lines new TypeScript across 2-3 files

### Phase 2 — Batch Migration (scales validated approach)

**Group 2.1: Backend — Batch Migration** (depends on Phase 1)

- [ ] Add `preview_batch_proton_migration()` → `Vec<MigrationPlan>` with pre-flight validation
- [ ] Add `apply_batch_proton_migration(plans, confirm: true)` → `Vec<MigrationResult>` with per-profile error reporting
- [ ] Pre-flight pass: serialize all profiles + verify all replacement paths before any writes
- [ ] Cross-family suggestions (Low confidence) excluded from batch operations by default
- [ ] Unit tests: batch partial failure, pre-flight rejection, mixed success/failure

**Group 2.2: Frontend — Batch Migration UX** (depends on 2.1)

- [ ] Add "Fix All Stale Proton Paths" button to Health Dashboard `TableToolbar` (visible when stale proton issues exist)
- [ ] Before/after review modal with per-profile checkboxes (NN/G confirmation dialog pattern)
- [ ] Descriptive confirm button: "Update N Profiles" not generic "Apply" (NN/G guideline)
- [ ] Cross-major suggestions excluded from batch by default; require opt-in per row
- [ ] Display batch results: "X of Y profiles migrated successfully" with per-profile failure details
- [ ] Post-migration: surface launcher re-export prompt for affected profiles (using existing `launcher_drift_map`)
- [ ] Auto-trigger `batchValidate()` to refresh all health statuses (no manual refresh)

**Group 2.3: Optional Enhancements** (deferrable)

- [ ] SQLite migration audit log table (`migration_log`)
- [ ] Proton version pinning (`proton_version_pinned` metadata flag to suppress suggestions)
- [ ] Profile-save-time inline suggestion in `ProtonPathField` (Option C additive)

### Estimated Total Scope

| Metric                     | Phase 1 (incl. prereqs)                             | Phase 2             | Total      |
| -------------------------- | --------------------------------------------------- | ------------------- | ---------- |
| **New Rust code**          | ~360 lines                                          | ~150 lines          | ~510 lines |
| **New TypeScript**         | ~180 lines                                          | ~220 lines          | ~400 lines |
| **Unit tests**             | ~170 lines                                          | ~80 lines           | ~250 lines |
| **New files**              | 2 (`profile/migration.rs`, `commands/migration.rs`) | 0 (extends Phase 1) | 2          |
| **Modified files**         | 5-6 (including prereq visibility changes)           | 2-3                 | 7-8        |
| **New crate dependencies** | 0                                                   | 0                   | 0          |

---

## Key Decisions Needed

1. **Same-family-only or cross-family suggestions?** Recommend same-family-only for Phase 1, cross-family as Phase 2 opt-in. Cross-family suggestions carry false-confidence risk (business-analyzer Risk 2) and should never be included in batch operations.

2. **Version comparison algorithm?** Recommend integer-tuple comparison (parse "9-4" → [9, 4]) over lexicographic sort. Lexicographic mis-orders "9-10" < "9-9" which would suggest a downgrade as "latest." (business-analyzer Risk 1)

3. **Proton version pinning?** Defer to Phase 2 optional enhancement. Phase 1 relies on explicit user confirmation per profile which is sufficient.

4. **Auto-migrate option?** No. Explicit confirmation is mandatory per security-researcher W-3. No writes from startup path — stale detection surfaces warnings only.

5. **Launcher re-export prompt?** Yes, in Phase 2. Use existing `launcher_drift_map` to identify affected launchers. Phase 1 can simply update the health status which will naturally surface drift.

6. **Write atomicity approach?** Write-to-temp + `fs::rename()` for all migration writes (security-researcher W-1). Standard `ProfileStore::save()` pattern remains unchanged for non-migration code paths.

---

## Open Questions

1. **How common are multi-family Proton setups?** If most users have one Proton family (e.g., only GE-Proton), same-family matching is almost always sufficient. If users mix GE-Proton + Proton Experimental, cross-family fallback becomes more important. This affects Phase 2 scoping.

2. **Steam root discovery for proton_run profiles** — Profiles using `runtime.proton_path` (not `steam.proton_path`) may lack a `steam.compatdata_path` from which to derive Steam root candidates. Should the migration tool scan default Steam root paths as fallback? (business-analyzer Risk 5)

3. **Steam Deck Flatpak paths** — Steam Deck uses Flatpak Steam at `~/.var/app/com.valvesoftware.Steam/data/Steam`. The existing `default_steam_client_install_path()` in `commands/steam.rs` already checks this path, but does `discover_compat_tools()` receive it as a steam root candidate? Verify coverage or add explicit support. (api-researcher)

4. **Integration with issue #38 (Health Dashboard)?** Is #38 complete or in-progress? This feature extends Health Dashboard capabilities and should coordinate with any ongoing #38 work.

5. **Should confidence scoring be visible to users?** The tech-designer proposed Exact/Family/Fuzzy confidence levels. ux-researcher recommends simplifying to "Recommended" (same family, newer) vs explicit warning (cross-major or cross-family). Defer complex scoring UI to Phase 2.

6. ~~**Local overrides in separate files?**~~ **Resolved.** security-researcher confirmed: local overrides are in the same TOML file under `[local_override]` — no separate file. Migration operates on one file per profile via `ProfileStore::save()`.

7. **Should `steam_applaunch` profiles skip stale-proton detection?** api-researcher notes that Steam itself manages compat tool assignments for `steam_applaunch` games. If Steam reassigns the compat tool, CrossHook's stored path becomes stale but Steam will use its own mapping anyway. Consider whether migration is only meaningful for `proton_run` profiles.

---

## Related Research Documents

- `docs/plans/proton-migration-tool/research-business.md` — Domain complexity, business rules, codebase readiness
- `docs/plans/proton-migration-tool/research-security.md` — Security risk assessment (0 CRITICAL, 4 WARNING [W-2 resolved], 10 ADVISORY)
- `docs/plans/proton-migration-tool/research-practices.md` — Reuse inventory, KISS assessment, module boundaries
- `docs/plans/proton-migration-tool/research-external.md` — External API/competitive analysis (Lutris, Heroic, Bottles, ProtonUp-Qt, Steam)
- `docs/plans/proton-migration-tool/research-ux.md` — Competitive UX analysis, interaction patterns, accessibility
