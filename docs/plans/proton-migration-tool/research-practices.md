# Practices Research: proton-migration-tool

## Executive Summary

The codebase has ~80% of the necessary infrastructure already in place. `discover_compat_tools` + `ProtonInstall.normalized_aliases` + the existing 3-tier matching logic in `steam/proton.rs` cover Proton discovery and fuzzy matching. `ProfileStore` covers profile CRUD. The only genuinely new logic needed is: (1) scanning profiles for stale proton paths, (2) a thin wrapper exposing the currently-private matching functions, and (3) a batch-save loop. No new crates are needed; the feature can ship as one new module (`profile/migration.rs`) and one Tauri command file.

**Post-review additions:** One implementation constraint confirmed by security-researcher: `ProfileStore::save()` is not crash-safe — migration writes SHOULD use write-to-temp + `fs::rename()` atomically (though not a blocker for P1 correctness). The `local_override` concern raised earlier was incorrect — see corrected Gotchas section for the full explanation of why the standard load→mutate→save pattern works correctly for portable profiles.

---

## Existing Reusable Code

| Module / Utility                                     | Location                                                | Purpose                                                                                                                       | How to Reuse                                                                                                                                                                                            |
| ---------------------------------------------------- | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `discover_compat_tools`                              | `steam/proton.rs:24`                                    | Scans Steam roots + system paths → `Vec<ProtonInstall>`                                                                       | Call directly with `steam_root_candidates`; already used by `list_proton_installs` command                                                                                                              |
| `discover_steam_root_candidates`                     | `steam/discovery.rs` (re-exported at `steam/mod.rs:14`) | Derives `Vec<PathBuf>` from a configured Steam path                                                                           | Already used in `commands/steam.rs:41`                                                                                                                                                                  |
| `ProtonInstall`                                      | `steam/models.rs:71`                                    | `name`, `path`, `is_official`, `aliases`, `normalized_aliases`                                                                | The `normalized_aliases: BTreeSet<String>` field is pre-computed — use directly for matching                                                                                                            |
| `normalize_alias` (private)                          | `steam/proton.rs:411`                                   | Strips to lowercase alphanumeric, e.g. `"GE-Proton 9-7"` → `"geproton97"`                                                     | Must promote to `pub(crate)` — the single gatekeeping function for all version comparison                                                                                                               |
| `resolve_compat_tool_by_name` (private)              | `steam/proton.rs:273`                                   | 3-tier exact→normalized→heuristic match against installed tools                                                               | Expose as `pub(crate) fn find_best_proton_replacement(stale_dir_name: &str, installed: &[ProtonInstall]) -> Vec<&ProtonInstall>`                                                                        |
| `ProfileStore::list` / `load` / `save`               | `profile/toml_store.rs:140,100,113`                     | Full profile CRUD                                                                                                             | Iterate `list()`, `load()` each profile to check paths, `save()` after update                                                                                                                           |
| `GameProfile` (all proton fields)                    | `profile/models.rs:32`                                  | `steam.proton_path`, `runtime.proton_path`, `local_override.steam.proton_path`, `local_override.runtime.proton_path`          | All four fields may hold stale paths — migration must handle all independently                                                                                                                          |
| `GameProfile::effective_profile` + `storage_profile` | `profile/models.rs:243,272`                             | `effective_profile` merges overrides into base; `storage_profile` inverts: copies base paths to `local_override`, clears base | `ProfileStore::load()` returns effective form; `save()` calls `storage_profile()` to reconstruct correct on-disk representation — the standard load→mutate→save roundtrip is safe for portable profiles |
| `check_profile_health` / `batch_check_health`        | `profile/health.rs:339,451`                             | Already detects missing `steam.proton_path` and `runtime.proton_path` as `Stale` issues                                       | Health check can seed the "which profiles need migration" list — filter on `field == "steam.proton_path"` or `"runtime.proton_path"`                                                                    |
| `HealthIssue.field`                                  | `profile/health.rs:31`                                  | Identifies which path field triggered the issue                                                                               | Use to detect proton staleness without reimplementing path checks                                                                                                                                       |
| `SyncSource` enum                                    | `metadata/profile_sync.rs:1`                            | Audit trail for profile writes                                                                                                | Needs a new `AppMigration` variant to properly track migration saves                                                                                                                                    |
| `observe_profile_write`                              | `metadata/profile_sync.rs:11`                           | Updates SQLite metadata after a profile write                                                                                 | Call after each migration save, same as `profile_save` command does at `commands/profile.rs:110`                                                                                                        |
| `sanitize_display_path`                              | `src-tauri/commands/shared.rs:20`                       | Replaces `$HOME` with `~` for display                                                                                         | Use when returning stale/new path strings over IPC                                                                                                                                                      |
| `default_steam_client_install_path`                  | `commands/steam.rs:9`                                   | Derives Steam root from env + filesystem                                                                                      | Reuse this logic (or extract to core) for migration command to get `steam_root_candidates`                                                                                                              |

---

## Modularity Design

### Recommended module boundaries

**New core module**: `crosshook-core/src/profile/migration.rs`

This is the right home because:

- The feature requires both `ProfileStore` (profile reads/writes) and `steam::proton` (discovery/matching) — it straddles both domains but the mutation target is a profile
- Placing it in `profile/` keeps `steam/` read-only infrastructure; migration is a profile concern
- Mirrors how `health.rs` lives in `profile/` despite checking paths from multiple sections

**Changes needed to `steam/proton.rs`**:

- Promote `normalize_alias` to `pub(crate)` (or `pub`)
- Add a new pub function `pub fn find_best_proton_matches(stale_dir_name: &str, installed: &[ProtonInstall]) -> Vec<ProtonInstall>` that wraps `resolve_compat_tool_by_name` — avoid making internal functions public by adding a thin adaptor

**New Tauri command file**: `src-tauri/src/commands/migration.rs`

- Two commands: `scan_stale_proton_paths` and `apply_proton_migration`
- Receives `State<'_, ProfileStore>` + `State<'_, MetadataStore>`, derives steam roots internally

**Shared between feature-specific and general**:

- `discover_compat_tools` and `discover_steam_root_candidates` — already `pub`, no changes needed
- `ProtonInstall` — already `pub` + `Serialize/Deserialize`, crosses IPC cleanly

---

## KISS Assessment

| Proposal aspect            | Complexity                                                             | Simpler alternative                                                 | Trade-off                                                                           |
| -------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Stale detection            | Low — reuse `batch_check_health` output, filter on proton fields       | Or skip health module, just check `Path::new(path).exists()` inline | Health module reuse adds enrichment; direct path check is 3 lines and zero coupling |
| Version suggestion         | Low — `normalize_alias` + digit extraction already implemented         | Just do `discover_compat_tools()` and present all installs unranked | Ranking is better UX but not required for P1; can ship unranked first               |
| Batch migration            | Low — iterate `list()`, mutate each profile, `save()`                  | Same                                                                | No simpler alternative                                                              |
| Rollback                   | **Not needed for P1** — confirmation dialog before apply is sufficient | Omit rollback                                                       | If user accepts wrong suggestion, they fix it in the profile editor                 |
| Per-profile approve/reject | Medium — requires returning a plan struct and a second IPC call        | Apply all or none                                                   | Per-profile granularity is stated in the spec; worth the extra IPC round-trip       |
| `SyncSource::AppMigration` | Trivial — add enum variant                                             | Reuse `AppWrite`                                                    | `AppWrite` is semantically wrong; add the variant for accurate history              |

**KISS verdict**: The proposal is well-scoped. Implement the full plan as described. The only place to simplify is to defer ranked suggestions (heuristic confidence scoring) and just show all discovered installs, letting the user pick — this saves ~30 lines of scoring code.

---

## Abstraction vs. Repetition

| Pattern                                          | Count in codebase                                       | Decision                                                                                                         |
| ------------------------------------------------ | ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| "scan profiles for path issues"                  | 1 (`batch_check_health`)                                | Do NOT abstract — migration has different output shape; use health module as a reference, not a dependency       |
| "save profile + notify metadata"                 | 5 instances across `commands/profile.rs`                | Already repetitive — but do NOT add a new abstraction layer for this feature; follow the existing inline pattern |
| `normalize_alias` logic (lowercase alphanumeric) | 1 place today                                           | Promote to `pub(crate)` rather than copy — this IS the second use, worth exposing                                |
| Path-exists check                                | Dozens of `fs::metadata()` calls throughout `health.rs` | Keep inline — no abstraction needed                                                                              |

**Rule of Three applied**: Only `normalize_alias` reaches the second-use threshold within this feature. All other patterns are either already abstracted or below the extraction threshold.

---

## Interface Design

### Core library (`crosshook-core/src/profile/migration.rs`)

```rust
/// Identifies which field in a profile holds a stale Proton path.
pub enum StaleProtonField {
    SteamProtonPath,        // effective profile.steam.proton_path
    RuntimeProtonPath,      // effective profile.runtime.proton_path
}

/// A profile whose Proton path no longer exists on disk.
pub struct StaleProtonProfile {
    pub name: String,
    pub stale_path: String,
    pub field: StaleProtonField,
}

/// A candidate replacement Proton for a stale profile.
pub struct ProtonMigrationCandidate {
    pub profile_name: String,
    pub field: StaleProtonField,
    pub stale_path: String,
    pub suggested_install: ProtonInstall,   // serializable, crosses IPC
}

/// Output of the scan phase.
pub struct ProtonMigrationPlan {
    pub candidates: Vec<ProtonMigrationCandidate>,
    pub unresolved: Vec<StaleProtonProfile>, // stale but no match found
}

/// Output of the apply phase.
pub struct ProtonMigrationResult {
    pub updated: Vec<String>,
    pub failed: Vec<(String, String)>,  // (profile_name, error)
}

pub fn scan_stale_proton_paths(store: &ProfileStore) -> Vec<StaleProtonProfile>;

pub fn find_migration_candidates(
    stale: &[StaleProtonProfile],
    installed: &[ProtonInstall],
) -> ProtonMigrationPlan;

pub fn apply_migration(
    store: &ProfileStore,
    approvals: &[(String, StaleProtonField, PathBuf)],  // (name, field, new_path)
) -> ProtonMigrationResult;
```

### Tauri IPC (`commands/migration.rs`)

```rust
#[tauri::command]
pub fn scan_proton_migration(
    store: State<'_, ProfileStore>,
    steam_client_install_path: Option<String>,
) -> Result<ProtonMigrationPlan, String>

#[tauri::command]
pub fn apply_proton_migration(
    approvals: Vec<ApprovedMigration>,  // {name, field, new_path}
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonMigrationResult, String>
```

**Key IPC note**: `scan_proton_migration` does NOT need `MetadataStore` — it only reads profiles and filesystem. Keep it stateless. `apply_proton_migration` needs `MetadataStore` only to call `observe_profile_write`.

---

## Testability Patterns

### Recommended patterns (follow existing codebase conventions)

1. **Filesystem isolation with `tempfile::tempdir()`** — already standard in `proton.rs` tests and `toml_store.rs` tests; use the same pattern for migration tests

2. **`ProfileStore::with_base_path(tmp)`** — use for isolated profile stores; already the standard test harness

3. **`create_tool(dir, vdf)` helper** — copy from `proton.rs:509` tests to create fake Proton installs at known paths

4. **Test `scan_stale_proton_paths` by writing a profile that references a deleted path** — create the path, save the profile, delete the path, then call scan. This matches `missing_proton_reports_stale_for_steam_applaunch` in `health.rs`.

5. **Test `find_migration_candidates` with known `Vec<ProtonInstall>`** — fully deterministic, no filesystem needed

6. **Test `apply_migration` with a temp store** — verify `store.load(name).steam.proton_path == new_path` after apply

### Anti-patterns to avoid

- **Do NOT mock `ProfileStore` or `ProtonInstall`** — they are lightweight and the codebase deliberately avoids mocking filesystem in tests
- **Do NOT test batch behavior with 50 profiles** — 2-3 profiles covers all paths
- **Do NOT assert on `checked_at` timestamps** — they are non-deterministic

---

## Build vs. Depend

External crate evaluation confirmed by api-researcher (see `research-external.md`):

| Need                           | Build vs. Library                  | Crate considered                                  | Recommendation                                                  | Rationale                                                                                         |
| ------------------------------ | ---------------------------------- | ------------------------------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Version string comparison      | **Build**                          | `semver` (rejected), `version-compare`            | Use `normalize_alias` (already implemented)                     | "GE-Proton9-4" → "geproton94" covers all known Proton naming; strict semver rejects these strings |
| Fuzzy name matching            | **Build**                          | `strsim` (not evaluated)                          | Expose `resolve_compat_tool_by_name` via thin wrapper           | 3-tier logic already exists in `steam/proton.rs:273`; promote to `pub(crate)`                     |
| Display-order sorting          | **Build (or `alphanumeric-sort`)** | `alphanumeric-sort` 1.5.x (zero deps, maintained) | Build for P1; reconsider if UI ordering complaints arise        | Digit-tuple comparison via `extract_numeric_segments` is ~15 lines                                |
| Path existence check           | **Build**                          | —                                                 | `std::fs::metadata()` + `ErrorKind::NotFound`                   | Already the pattern throughout `health.rs`                                                        |
| Profile TOML mutation          | **Build**                          | —                                                 | `ProfileStore::load` + atomic save (see below)                  | Zero new dependencies; **note:** `save()` is NOT currently atomic — see Gotchas                   |
| Proton install/download        | **Reject**                         | `libprotonup` 0.9.1                               | Omit entirely                                                   | Downloads/installs Proton — far out of scope for path migration                                   |
| Migration result serialization | **Build**                          | —                                                 | `#[derive(Serialize, Deserialize)]` + `serde` (already in tree) | No additions                                                                                      |
| Transaction/rollback           | **Not needed**                     | —                                                 | Omit                                                            | Confirmation dialog before apply is sufficient for P1                                             |

**Verdict**: Zero new crate dependencies for MVP. All required logic exists in the codebase or the standard library.

---

## Frontend Component Reuse (from UX research)

UX researcher identified several component opportunities. Verification against actual source:

| Component / Pattern                                                           | Status                    | Location                                                           | Reuse verdict                                                                                                                                                                                              |
| ----------------------------------------------------------------------------- | ------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Modal shell (portal + focus trap + backdrop + ARIA)                           | ✅ Confirmed              | `LauncherPreviewModal.tsx:51-303`                                  | **Directly reusable** — copy the shell, replace body/footer content. Has Escape handler, Tab trap, aria-modal, focus restore on close, `inert` management for background nodes                             |
| `CollapsibleSection`                                                          | ✅ Confirmed exported     | `components/ui/CollapsibleSection.tsx`                             | **Directly reusable** — accepts `title`, `meta` (React node slot), `open`/`onToggle` for controlled mode. Use for "Show full paths" in migration modal                                                     |
| `HealthBadge`                                                                 | ✅ Confirmed exported     | `components/HealthBadge.tsx`                                       | **Reusable** — accepts `onClick` for interactive mode, renders `crosshook-status-chip` pattern. Use to show current health status in migration table rows                                                  |
| `TableToolbar` (filter chips + search + count + button)                       | ⚠️ File-local             | `HealthDashboardPage.tsx:110-183`                                  | **Pattern reusable, not importable** — `TableToolbar` is not exported. To add a "Fix Proton Paths" button, modify the existing `TableToolbar` in `HealthDashboardPage.tsx` directly rather than extracting |
| CSS variables `--crosshook-color-warning` / `--crosshook-color-danger`        | ✅ Confirmed              | `styles/variables.css:17-18`                                       | `#f5c542` and `#ff758f` — use for inline stale path warnings and broken path cells                                                                                                                         |
| `.crosshook-focus-target` / `.crosshook-nav-target` / `.crosshook-focus-ring` | ✅ Confirmed              | `styles/focus.css`                                                 | All new interactive elements (checkboxes, migration confirm button) must use these classes for controller mode                                                                                             |
| Inline field warning / suggestion pattern                                     | ❌ Does not exist         | Searched `ProfileFormSections.tsx` — no matches                    | A `<FieldWarning suggestion={...} />` component is **genuinely new**; the UX researcher's suggestion to build it generically is worth doing if it would apply to executable path warnings too              |
| Linear progress bar for batch ops                                             | ❌ Does not exist         | Only `PageBanner.tsx` has any progress reference (loading spinner) | No progress bar component in the codebase — use a CSS-only `<progress>` element or minimal inline style; don't add a component library for one bar                                                         |
| `categorizeIssue` / `IssueCategory` enum                                      | ✅ Confirmed (file-local) | `HealthDashboardPage.tsx:39-48`                                    | Already maps `field === 'steam.proton_path'` → `'missing_proton'` category — the migration trigger should reuse this categorization logic (extract to a shared util if needed)                             |

### Key finding for implementation

`LauncherPreviewModal` is the correct modal pattern to follow — not `ProfileReviewModal`. The `ProfileReviewModal` has a summary-item layout (label + value pairs) but no checkboxes or before/after table. The migration modal needs new body content but can inherit the full accessibility shell verbatim from `LauncherPreviewModal`.

---

## Open Questions

1. **`local_override` handling (NOT a blocker — corrected)**: Earlier analysis incorrectly flagged this as a blocker. The actual data flow is safe: `load()` calls `effective_profile()` which merges `local_override` into the base fields (e.g., `steam.proton_path = "/old/..."`) then clears `local_override`. After migration updates `steam.proton_path = "/new/..."`, `save()` calls `storage_profile()` which copies ALL base path fields into `local_override` and clears the base fields before writing. Result on disk: `{steam.proton_path: "", local_override.steam.proton_path: "/new/..."}` — exactly correct. The `storage_profile_roundtrip_is_idempotent` test at `models.rs:492` proves this. The standard load→mutate→save pattern used by all existing profile mutations is correct and sufficient.

2. **Atomic writes (quality concern, not correctness blocker)**: `ProfileStore::save()` calls `fs::write()` directly — not crash-safe. For a batch migration of N profiles, a crash mid-batch would leave some profiles updated and others not, with no corrupted files (the OS write either completes or doesn't for small files). For P1 this is acceptable; a future improvement would use write-to-temp + `fs::rename()`.

3. **Module placement (open debate)**: tech-designer proposes `steam/migration.rs`; this report recommends `profile/migration.rs`. business-analyzer splits the difference: pure suggestion algorithm in `steam/proton.rs`, plan/apply in `profile/migration.rs`. **Recommendation**: keep pure proton discovery/matching in `steam/`, put the "scan profiles → build plan → apply" orchestration in `profile/migration.rs`. This preserves `steam/` as read-only infrastructure.

4. **Confidence tier UX**: Should the UI distinguish between "exact match found" vs. "heuristic match" so users know when to be skeptical? A `MatchConfidence` enum on `ProtonMigrationCandidate` costs little and avoids a follow-up issue.

5. **`SyncSource` enum visibility**: Check whether adding `AppMigration` variant requires changes to `lib.rs` exports. Look at how `commands/profile.rs` passes `SyncSource::AppWrite` — if `SyncSource` is `pub(crate)`, migration in `profile/migration.rs` can use it directly; Tauri command layer inherits.

6. **Multiple stale proton fields per profile**: A single profile could have both `steam.proton_path` AND `runtime.proton_path` stale. The plan struct should represent multiple candidates per profile, not one-to-one.

7. **Proton-TKG naming breaks digit-based version ranking**: TKG tools use `proton_tkg_X.Y.rN.gHASH.release` naming — the git commit hash (`gHASH`) makes digit extraction produce meaningless rank scores. Detection: check `normalized_aliases.iter().any(|a| a.starts_with("protontkg"))`. Handling: exclude TKG tools from numeric ranking but still include them in the candidate list as unranked entries for manual user selection. No new fields on `ProtonInstall` needed — `is_official: false` + family prefix check is sufficient. The same principle applies to any future hash-versioned tool family.
