# Feature Spec: Proton Migration Tool

## Executive Summary

When users upgrade Proton (e.g., GE-Proton 9-4 to 9-7), CrossHook profiles referencing the old path break silently with a generic "path does not exist" error -- a recurring pain point given GE-Proton's weekly release cadence. This feature adds a **Proton version migration tool** that detects stale Proton paths in profiles, suggests the closest same-family replacement using a family-based fuzzy matching algorithm built on the existing `discover_compat_tools()` and `normalize_alias()` pipeline, and lets users apply updates individually (inline) or in batch (review modal) with explicit before/after confirmation. No existing Linux game launcher (Lutris, Heroic, Bottles, ProtonUp-Qt) implements proactive stale-path detection or guided migration -- CrossHook would be first-in-class. The implementation requires zero new crate dependencies, adds one new Rust module (`steam/migration.rs`), one Tauri command file, and Health Dashboard UI enhancements, shipping in two phases: Phase 1 validates the algorithm with single-profile migration; Phase 2 adds batch migration with per-profile deselection.

**Source**: [GitHub issue #48](https://github.com/yandy-r/crosshook/issues/48)

---

## External Dependencies

### APIs and Services

**None.** This feature operates entirely on the local filesystem -- scanning Steam library directories for installed Proton versions and modifying TOML profile files in `~/.config/crosshook/`. No network I/O, no external APIs.

### Libraries and SDKs

| Library             | Status   | Purpose                     | Decision                                        |
| ------------------- | -------- | --------------------------- | ----------------------------------------------- |
| `semver`            | Rejected | SemVer parsing              | Rejects all Proton naming formats               |
| `version-compare`   | Rejected | Fuzzy version comparison    | Unreliable on Proton strings                    |
| `alphanumeric-sort` | Optional | Natural sort for UI display | Custom digit extractor preferred                |
| `libprotonup`       | Rejected | Proton download/install     | Out of scope -- CrossHook already has discovery |

**Verdict: Zero new crate dependencies.** All needed logic exists in the standard library and existing CrossHook modules.

### External Documentation

- [GE-Proton releases](https://github.com/gloriouseggroll/proton-ge-custom/releases): Release cadence, naming conventions
- [Valve/Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ): Official Proton filesystem layout
- [Flathub Steam wiki](https://github.com/flathub/com.valvesoftware.Steam/wiki): Flatpak Steam path differences

---

## Business Requirements

### User Stories

**Primary Users: Steam Deck and Linux Desktop Gamers**

| ID   | As a...                    | I want to...                                                                     | So that...                                                        |
| ---- | -------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| US-1 | Steam Deck user            | see a clear message that my GE-Proton was upgraded and my profile needs updating | I don't guess why my game won't launch                            |
| US-2 | Linux gamer                | get a suggested replacement Proton from the same family                          | I don't browse deep filesystem paths manually                     |
| US-3 | Power user                 | migrate all broken profiles at once                                              | I don't repeat the same fix for 10 profiles after a Proton update |
| US-4 | Community profile importer | see a candidate from the same family even with a newer local build               | the imported profile works without manual path hunting            |
| US-5 | Any user                   | review before/after paths before they're written                                 | I don't accidentally downgrade a working Proton                   |
| US-6 | Careful user               | be warned when no suitable replacement exists                                    | I know I need to install the right Proton                         |
| US-7 | Any user                   | be warned when the only replacement is a different major version                 | I can decide whether to risk a prefix migration                   |

### Business Rules

**BR-1 -- Stale Detection Trigger**: A profile has a stale Proton path when the effective executable path returns `NotFound` via `try_exists()`. Applies to both `steam.proton_path` (steam_applaunch) and `runtime.proton_path` (proton_run). `native` launch method profiles are excluded.

**BR-2 -- Method-Aware Path Targeting**: Migration targets the correct field based on effective launch method: `steam_applaunch` -> `steam.proton_path`, `proton_run` -> `runtime.proton_path`.

**BR-3 -- Local Override Layer Awareness**: Migration uses the standard `load()` -> modify effective -> `save()` cycle. `storage_profile()` automatically routes machine-local paths to `local_override`. No raw TOML manipulation.

**BR-4 -- Version Suggestion Strategy**: Select the **latest available** build within the highest-confidence tier:

1. Same family, same major, highest build (default selection, no warning)
2. Same family, higher major (requires warning; excluded from batch default)
3. Same family, older build (downgrade warning; excluded from batch default)
4. Different family fallback (cross-family disclaimer; requires per-profile opt-in)

**BR-5 -- User Confirmation Model**: Single-profile inline fix = one-click Apply + undo toast (5s). Batch migration (>=2 profiles) = review modal with before/after pairs, per-profile deselection, explicit "Update N Profiles" button.

**BR-6 -- Batch Pre-Flight Validation**: Before writing any file, serialize all migration targets to TOML and validate each new path exists. If any pre-flight fails, abort with zero writes.

**BR-7 -- No Downgrade Without Warning**: Older-build suggestions flagged visually. Cross-major suggestions excluded from batch defaults.

**BR-8 -- Zero Matches State**: When no Proton is installed at all, show informational message with "Browse manually" CTA and ProtonUp-Qt reference. No writes.

**BR-9 -- Cross-Family Never Silent**: Official Proton must never silently replace GE-Proton (or vice versa). Cross-family = explicit per-profile opt-in only.

**BR-10 -- "Proton Experimental" Is Not Version-Comparable**: Rolling release, no version number. Can only auto-suggest another "Proton Experimental" install.

**BR-11 -- Post-Migration Metadata Sync**: Every migration write calls `observe_profile_write()` with `SyncSource::AppMigration` to keep the SQLite metadata registry in sync.

### Edge Cases

| Scenario                                                | Expected Behavior                                                                |
| ------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Multiple profiles pointing to same stale path           | All included in batch; migrated together                                         |
| Profile with both proton fields stale                   | Each field treated independently in scan                                         |
| Community tap profiles (portable)                       | `local_override` handles machine-local paths transparently                       |
| Legacy GE naming (`Proton-9.23-GE-2`)                   | Phase 1: treated as separate family from modern GE (conservative)                |
| "Proton Experimental" path stale                        | No version comparison; suggest only another Experimental install                 |
| Proton-TKG builds                                       | Excluded from version ranking (git hash in name); shown in manual list           |
| Version "9-10" vs "9-9" ordering                        | Integer-tuple comparison, not lexicographic                                      |
| Replacement uninstalled between scan and apply (TOCTOU) | Re-validate path at apply time; fail gracefully with re-scan prompt              |
| Exported launcher scripts after migration               | Existing `DriftState` tracking detects stale launchers; surface re-export notice |

### Success Criteria

- [ ] Profiles with stale Proton paths show specific migration suggestions on Health Dashboard
- [ ] Single-profile inline migration applies in one click with undo toast
- [ ] Batch migration (>=2 profiles) shows review modal with safe/needs-review sections
- [ ] Cross-family and cross-major suggestions excluded from batch defaults
- [ ] "No match" state shown with "Browse manually" CTA when no same-family replacement exists
- [ ] Each migration logged via `observe_profile_write()` with `SyncSource::AppMigration`
- [ ] Feature integrates into existing Health Dashboard without requiring a new page

---

## Technical Specifications

### Architecture Overview

```text
Frontend (React/TypeScript)
  useProtonMigration.ts ----invoke----> commands/migration.rs (Tauri IPC)
  useProfileHealth.ts   ----invoke----> commands/health.rs    (existing)

Tauri IPC Layer
  commands/migration.rs
    |-- check_proton_migrations()    -> MigrationScanResult
    |-- apply_proton_migration()     -> MigrationApplyResult
    +-- apply_batch_migration()      -> BatchMigrationResult

crosshook-core (Rust library)
  steam/migration.rs (NEW)
    |-- scan_proton_migrations()     <- uses steam/proton.rs discovery
    |-- apply_single_migration()     <- uses profile/toml_store.rs save
    +-- extract_proton_family()      <- uses normalize_alias (promoted to pub(crate))
  steam/proton.rs (MODIFIED -- visibility only)
    |-- discover_compat_tools()      [reused]
    +-- normalize_alias()            [promoted to pub(crate)]
  profile/toml_store.rs (existing)
    |-- ProfileStore::load()         [reused]
    +-- ProfileStore::save()         [reused]
```

### Data Models

#### Rust Structs (`steam/migration.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtonPathField {
    SteamProtonPath,
    RuntimeProtonPath,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSuggestion {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub old_path: String,
    pub new_path: String,
    pub old_proton_name: String,
    pub new_proton_name: String,
    pub confidence: f64,           // 0.0..=1.0
    pub proton_family: String,
    pub crosses_major_version: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmatchedProfile {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub stale_path: String,
    pub stale_proton_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationScanResult {
    pub suggestions: Vec<MigrationSuggestion>,
    pub unmatched: Vec<UnmatchedProfile>,
    pub profiles_scanned: usize,
    pub affected_count: usize,
    pub installed_proton_versions: Vec<ProtonInstallInfo>,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationOutcome { Applied, AlreadyValid, Failed }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationApplyResult {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub old_path: String,
    pub new_path: String,
    pub outcome: MigrationOutcome,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMigrationResult {
    pub results: Vec<MigrationApplyResult>,
    pub applied_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
}
```

#### TypeScript Types (`src/types/migration.ts`)

```typescript
export type ProtonPathField = 'steam_proton_path' | 'runtime_proton_path';
export type MigrationOutcome = 'applied' | 'already_valid' | 'failed';

export interface MigrationSuggestion {
  profile_name: string;
  field: ProtonPathField;
  old_path: string;
  new_path: string;
  old_proton_name: string;
  new_proton_name: string;
  confidence: number;
  proton_family: string;
  crosses_major_version: boolean;
}

export interface MigrationScanResult {
  suggestions: MigrationSuggestion[];
  unmatched: UnmatchedProfile[];
  profiles_scanned: number;
  affected_count: number;
  installed_proton_versions: ProtonInstallInfo[];
  diagnostics: string[];
}
```

### Core Algorithm: Family-Based Matching

**Step 1 -- Extract Family**: Normalize name via `normalize_alias()`, strip trailing digits. `"GE-Proton9-7"` -> `"geproton97"` -> family `"geproton"`.

**Step 2 -- Extract Version**: Split on non-digit boundaries, parse as integer tuple. `"GE-Proton10-34"` -> `[10, 34]`. Compare as tuples (`[10, 34] > [9, 7]`).

**Step 3 -- Rank Candidates**: Within same family, score by version relationship:

| Scenario                               | Confidence | Batch Default       |
| -------------------------------------- | ---------- | ------------------- |
| Same family, same major, newer build   | 0.9        | Checked             |
| Same family, newer major version       | 0.75       | Unchecked (warning) |
| Same family, same major, older build   | 0.7        | Unchecked (warning) |
| Same family, older major version       | 0.5        | Unchecked (warning) |
| Versionless match (e.g., Experimental) | 0.8        | Checked             |

### API Design

#### `check_proton_migrations` (read-only scan)

```rust
#[tauri::command]
pub fn check_proton_migrations(
    steam_client_install_path: Option<String>,
    store: State<'_, ProfileStore>,
) -> Result<MigrationScanResult, String>
```

Frontend: `invoke<MigrationScanResult>('check_proton_migrations', { steamClientInstallPath: null })`

#### `apply_proton_migration` (single profile write)

```rust
#[tauri::command]
pub fn apply_proton_migration(
    request: ApplyMigrationRequest,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<MigrationApplyResult, String>
```

#### `apply_batch_migration` (multi-profile write, Phase 2)

```rust
#[tauri::command]
pub fn apply_batch_migration(
    request: BatchMigrationRequest,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<BatchMigrationResult, String>
```

### System Integration

#### Files to Create

| File                                           | Purpose                                  |
| ---------------------------------------------- | ---------------------------------------- |
| `crates/crosshook-core/src/steam/migration.rs` | Core migration logic: scan, match, apply |
| `src-tauri/src/commands/migration.rs`          | Tauri IPC command handlers               |
| `src/types/migration.ts`                       | TypeScript type definitions              |
| `src/hooks/useProtonMigration.ts`              | React hook for migration state           |

#### Files to Modify

| File                                           | Change                                     |
| ---------------------------------------------- | ------------------------------------------ |
| `crates/crosshook-core/src/steam/mod.rs`       | Add `pub mod migration;`                   |
| `crates/crosshook-core/src/steam/proton.rs`    | Promote `normalize_alias` to `pub(crate)`  |
| `crates/crosshook-core/src/metadata/models.rs` | Add `AppMigration` variant to `SyncSource` |
| `src-tauri/src/commands/mod.rs`                | Add `pub mod migration;`                   |
| `src-tauri/src/lib.rs`                         | Register migration commands                |
| `src/types/index.ts`                           | Re-export migration types                  |
| `src/components/pages/HealthDashboardPage.tsx` | Add migration action buttons               |

---

## UX Considerations

### User Workflows

#### Primary Flow -- Health Dashboard Batch (Phase 2)

1. **Discovery**: Health Dashboard loads, detects N profiles with `missing_proton` issues
2. **Trigger**: "Fix Proton Paths (N)" toolbar button appears
3. **Scan**: Click triggers `check_proton_migrations` -> spinner "Scanning Proton installations..."
4. **Review**: Migration Review Modal shows before/after table with checkboxes:
   - Safe rows (same-family, same-major): pre-checked, green indicator
   - Cross-major rows: unchecked, amber warning
   - Different-family rows: collapsed section, requires explicit opt-in
   - No-suggestion rows: separate section, no checkbox
5. **Confirm**: User clicks "Update N Profiles" -> sequential writes with progress
6. **Result**: Summary "5 updated, 1 needs manual attention" -> health auto-refreshes

#### Single Profile Inline Fix (Phase 1)

1. Health Dashboard row shows stale Proton issue with "Update Proton" action
2. Inline suggestion: "GE-Proton 9-7 found [Use GE-Proton 9-7] [Choose different...]"
3. One click -> profile updated -> undo toast (5s) -> health badge refreshes

#### Profile Editor Inline (Additive Enhancement)

1. Proton path field shows error icon + "GE-Proton 9-4 is no longer installed"
2. Suggestion below field: "GE-Proton 9-7 found [Use GE-Proton 9-7] [Browse...]"
3. Click updates field value; user saves normally

### UI Patterns

| Component              | Pattern                                                                                             | Notes                                      |
| ---------------------- | --------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| Migration Review Modal | `LauncherPreviewModal` shell (portal + focus trap + ARIA)                                           | New body content; NOT `ProfileReviewModal` |
| Collapsed path details | `CollapsibleSection`                                                                                | For "Show full path" expand                |
| Health status in rows  | `HealthBadge`                                                                                       | Reuse with `onClick` for interactive mode  |
| Batch toolbar button   | Extend existing `TableToolbar` in HealthDashboardPage                                               | File-local component, modify in place      |
| Confidence badges      | CSS variables: `--crosshook-color-success`, `--crosshook-color-warning`, `--crosshook-color-danger` | Green/amber/red per confidence tier        |
| Inline field warning   | New `<FieldWarning>` component                                                                      | Genuinely new; no existing equivalent      |
| Progress bar (batch)   | CSS-only `<progress>` element                                                                       | No component library needed                |

### Accessibility / Steam Deck

- All interactive elements use `crosshook-focus-ring`, `crosshook-focus-target`, `crosshook-nav-target` CSS classes
- Minimum touch target: `var(--crosshook-touch-target-min)` (48px desktop / 56px controller)
- Modal tab order: Select All -> row checkboxes -> Cancel -> Update N Profiles
- `useGamepadNav` hook applied to migration modal focus scope
- Controller prompts: "A: Toggle B: Cancel Start: Confirm"

### Performance UX

- **Scan phase**: Modal opens immediately with spinner; if >3s, show "Taking longer than usual..."
- **Apply phase**: Progress bar for >=3 profiles; spinner for 1-2
- **Post-migration**: Programmatic health re-check (no user prompt needed)
- **No optimistic updates**: Wait for filesystem write confirmation before updating React state

### Error Handling

| State                               | UI Pattern             | Message                                                                 |
| ----------------------------------- | ---------------------- | ----------------------------------------------------------------------- |
| Single path, same-family suggestion | Inline below field     | "GE-Proton 9-4 not found. GE-Proton 9-7 available. [Use GE-Proton 9-7]" |
| Single path, no suggestion          | Inline with browse CTA | "No Proton installations detected. [Browse...] [Install Proton ->]"     |
| Batch partial failure               | Post-migration result  | "3 updated, 1 failed: Dark Souls III -- no writable path"               |
| TOCTOU: suggested version deleted   | Error + re-scan CTA    | "GE-Proton 9-7 no longer available. [Re-scan]"                          |
| Cross-major suggestion              | Per-row inline warning | "Major version change -- prefix may need recreation"                    |

### Language and Tone

| Instead of                             | Use                                                               |
| -------------------------------------- | ----------------------------------------------------------------- |
| "Error: Proton path invalid"           | "GE-Proton 9-4 is no longer installed"                            |
| "The Steam Proton path does not exist" | "This Proton version was removed. A migration tool is available." |
| "Apply" / "OK"                         | "Update 4 Profiles"                                               |

---

## Recommendations

### Implementation Approach

**Recommended Strategy**: Separate migration module (`steam/migration.rs`) + Health Dashboard integration. Two-phase delivery to validate algorithm and UX incrementally.

### Prerequisites (Must Address Before Phase 1)

1. **Crash-safe writes**: Migration write path uses `write-to-temp + fs::rename()` for atomicity
2. **local_override correctness**: Migration uses `load()` -> modify effective -> `save()` roundtrip (confirmed safe by `storage_profile_roundtrip_is_idempotent` test)
3. **Promote private functions**: `normalize_alias()` and `resolve_compat_tool_by_name()` to `pub(crate)`

### Technology Decisions

| Decision           | Choice                                    | Rationale                                                                                  |
| ------------------ | ----------------------------------------- | ------------------------------------------------------------------------------------------ |
| Module placement   | `steam/migration.rs`                      | Core logic is Proton family matching (steam concern); profile load/save is a thin consumer |
| Matching algorithm | Family + integer-tuple version comparison | Handles all known Proton naming; prevents "9-10" < "9-9" bug                               |
| Batch atomicity    | Best-effort per-profile                   | Matches existing `batch_check_health` pattern                                              |
| Health integration | Separate IPC, frontend joins              | Health scan stays fast; migration queried on-demand                                        |
| Version comparison | Custom digit extractor (no new crate)     | ~15 lines; zero dependency risk                                                            |
| Write safety       | `fs::rename()` for migration path         | POSIX atomic on same FS; `.toml.tmp` then rename                                           |

### Quick Wins

1. **Improve remediation text now** -- Update `check_required_executable` in `health.rs` from generic "Re-browse" to: "Proton version not found. Check for updated versions in the Proton Path dropdown."
2. **Promote `normalize_alias` to `pub(crate)`** -- ~5 line visibility change, enables the entire suggestion engine
3. **Add `AppMigration` variant to `SyncSource`** -- ~3 lines, proper audit trail for migration writes

### Future Enhancements

- **Auto-heal on startup**: Toast "3 profiles have outdated Proton paths" linking to Health Dashboard (warning only, no auto-writes)
- **Proton version pinning**: `proton_version_pinned` metadata flag to suppress migration suggestions
- **Migration history in SQLite**: `migration_events` table for audit trail and undo capability
- **Launcher re-export prompt**: Detect affected exported launchers via `launcher_drift_map`
- **Cross-family suggestions**: Phase 2 opt-in for GE-Proton -> Official Proton fallback
- **Legacy GE alias table**: Unify `Proton-X.Y-GE-Z` and `GE-ProtonX-Y` as same family

---

## Risk Assessment

### Technical Risks

| Risk                                         | Likelihood | Impact | Mitigation                                                       |
| -------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------- |
| Version matching suggests wrong family       | Medium     | Medium | Family-prefix extraction; cross-family excluded from batch       |
| Version string mis-ordering ("9-10" < "9-9") | High       | Medium | Integer-tuple parsing, not lexicographic sort                    |
| Cross-major upgrade corrupts WINE prefix     | Medium     | Medium | Explicit warning; excluded from batch defaults                   |
| Local override layer bypassed                | Medium     | High   | Confirmed safe via load/save roundtrip; mandatory test           |
| Concurrent save race                         | Low        | Low    | Write-to-temp + rename; last-write-wins consistent with codebase |
| Flatpak Steam paths not discovered           | Low        | Medium | Prerequisite fix in `steam/discovery.rs`                         |
| Exotic naming (TKG, SteamTinker) mis-ranked  | Low        | Low    | Excluded from auto-ranking; shown in manual list                 |

### Security Considerations

#### Critical -- Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | --   | --                  |

#### Warnings -- Must Address

| Finding                                                     | Risk                                          | Mitigation                                                    |
| ----------------------------------------------------------- | --------------------------------------------- | ------------------------------------------------------------- |
| W-1: Non-atomic `fs::write()` in `ProfileStore::save()`     | Crash during batch = partial corruption       | Write to `.toml.tmp` then `fs::rename()` in migration path    |
| W-2: `steam.proton_path` empty in stored profiles (v0.2.4+) | Migration targeting base field silently fails | Use `load()` -> modify effective -> `save()` roundtrip        |
| W-3: No consent gate at command layer                       | Auto-migration without user approval          | Scan/apply command split; preview returns plan with no writes |
| W-4: No rollback for partial batch failure                  | Profiles 1-5 migrated, 6+ not                 | Pre-flight validation pass before any writes                  |

#### Advisories -- Best Practices

- **A-1**: Add `is_executable()` check on replacement candidates (deferrable -- discovery already verifies `proton` file)
- **A-2**: Canonicalize paths before dedup to avoid symlink duplicates (deferrable)
- **A-3**: SQLite `migration_log` table for audit trail (Phase 2)
- **A-4**: Apply `sanitize_display_path()` to all IPC-bound path strings (Phase 1 -- mandatory for consistency)
- **A-5**: Validate `steam_client_install_path` IPC argument (cheap, include in Phase 1)
- **A-8**: Use `try_exists()` not `exists()` for staleness check (distinguishes NotFound from permission error)
- **A-10**: No `dangerouslySetInnerHTML` for path display (React auto-escaping sufficient)

---

## Task Breakdown Preview

### Phase 1: Single-Profile Migration (validates algorithm + UX)

**Focus**: Core version suggestion engine + single-profile apply + Health Dashboard inline fix

**Group 1.1: Backend -- Version Suggestion Engine** (no dependencies)

- Create `steam/migration.rs` with `extract_proton_family()`, `extract_version_segments()`, `find_best_replacement()`
- Promote `normalize_alias()` to `pub(crate)` in `steam/proton.rs`
- Add `AppMigration` variant to `SyncSource` enum
- Unit tests: family extraction, version ordering, same-family newer/older, cross-family exclusion, TKG handling
- **~150 lines Rust + tests**

**Group 1.2: Backend -- Migration Tauri Commands** (depends on 1.1)

- Create `commands/migration.rs` with `check_proton_migrations`, `apply_proton_migration`
- Crash-safe writes: `.toml.tmp` + `fs::rename()` in migration write path
- `resolve_launch_method()` to target correct field
- `observe_profile_write()` with `SyncSource::AppMigration`
- Re-validate replacement path at apply time (TOCTOU mitigation)
- Tests: local_override correctness, method-specific targeting, stale replacement rejection
- **~200 lines Rust + Tauri wiring**

**Group 1.3: Frontend -- Single-Profile Migration UX** (depends on 1.2)

- Add `MigrationSuggestion`, `MigrationScanResult` TypeScript types
- Health Dashboard per-row "Suggest Fix" button for `missing_proton` issues
- Inline before/after display with confidence indicator
- "Apply" calls `apply_proton_migration` -> `revalidateSingle()` on success
- `sanitize_display_path()` pattern for displayed paths
- **~150 lines TypeScript**

**Parallelization**: Groups 1.1 backend work can start immediately. 1.3 frontend can begin type definitions in parallel.

### Phase 2: Batch Migration (scales validated approach)

**Focus**: Multi-profile batch flow + review modal + post-migration notices
**Dependencies**: Phase 1 complete

**Group 2.1: Backend -- Batch Commands**

- `apply_batch_migration` command with pre-flight validation pass
- Cross-family suggestions excluded from batch default
- Per-profile error isolation
- Tests: batch partial failure, pre-flight rejection

**Group 2.2: Frontend -- Batch Migration UX**

- "Fix All Stale Proton Paths" toolbar button in Health Dashboard
- Migration Review Modal (using `LauncherPreviewModal` shell): before/after table, checkboxes, confidence badges, "Needs Manual Review" collapsed section
- Progress bar for >=3 profiles
- Post-migration launcher re-export notice
- Auto-trigger `batchValidate()` after completion

### Estimated Total Scope

| Metric           | Phase 1    | Phase 2    | Total      |
| ---------------- | ---------- | ---------- | ---------- |
| New Rust code    | ~350 lines | ~150 lines | ~500 lines |
| New TypeScript   | ~150 lines | ~200 lines | ~350 lines |
| Unit tests       | ~150 lines | ~80 lines  | ~230 lines |
| New files        | 4          | 0          | 4          |
| Modified files   | 6          | 2-3        | 7-8        |
| New dependencies | 0          | 0          | 0          |

---

## Decisions Needed

1. **Version suggestion preference**
   - Options: Latest within family (recommended) vs. closest version number
   - Impact: Determines which Proton version is pre-selected for the user
   - Recommendation: Latest within same-family-same-major -- users most likely have the newest build

2. **Module placement**
   - Options: `steam/migration.rs` (recommended) vs. `profile/migration.rs`
   - Impact: Dependency direction and module responsibility boundaries
   - Recommendation: `steam/migration.rs` -- core logic is Proton family matching, a steam concern

3. **Confidence scoring visibility**
   - Options: Show numeric scores vs. simplified "Recommended"/"Alternative" labels
   - Impact: UI complexity and user comprehension
   - Recommendation: Simplified labels with optional expand for details

4. **Flatpak Steam path support**
   - Options: Fix in `steam/discovery.rs` as prerequisite vs. track separately
   - Impact: Flatpak users would miss migration suggestions without the fix
   - Recommendation: Parallel fix -- affects all Steam discovery, not just migration

5. **Legacy GE naming unification**
   - Options: Phase 1 conservative (separate families) vs. heuristic unification
   - Impact: Legacy GE-Proton users see "no match" instead of modern GE suggestion
   - Recommendation: Phase 1 conservative; add alias table in Phase 2 if users report issues

---

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Proton versioning schemes, crate evaluation, competitive launcher analysis
- [research-business.md](./research-business.md): User stories, business rules, workflows, domain model, codebase integration
- [research-technical.md](./research-technical.md): Architecture, data models, Rust/TS type definitions, matching algorithm, Tauri commands
- [research-ux.md](./research-ux.md): User workflows, UI patterns, gamepad support, competitive analysis, error handling
- [research-security.md](./research-security.md): 0 CRITICAL / 4 WARNING / 10 ADVISORY findings with mitigations
- [research-practices.md](./research-practices.md): Reuse inventory, KISS assessment, module boundaries, testability
- [research-recommendations.md](./research-recommendations.md): Phasing strategy, risk assessment, alternative approaches, task breakdown
