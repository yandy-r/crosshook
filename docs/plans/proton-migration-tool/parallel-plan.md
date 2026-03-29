# Proton Migration Tool Implementation Plan

This plan implements a Proton version migration tool that detects stale Proton paths in CrossHook profiles and suggests same-family replacements via a family-based fuzzy matching algorithm. The implementation adds one new Rust module (`profile/migration.rs`) consuming the existing `discover_compat_tools()` and `normalize_alias()` pipeline, three Tauri IPC commands with a dry-run/confirm consent gate, and Health Dashboard UI enhancements for inline single-profile fixes (Phase 1) and batch migration with a review modal (Phase 2). Zero new crate dependencies are required; the total scope is ~500 lines Rust + ~350 lines TypeScript across 4 new files and 8 modified files.

## Critically Relevant Files and Documentation

- docs/plans/proton-migration-tool/feature-spec.md: Complete feature specification with business rules, data models, API contracts, UX specs, and security requirements
- docs/plans/proton-migration-tool/research-technical.md: Detailed Rust struct definitions, matching algorithm with code examples, Tauri command signatures
- docs/plans/proton-migration-tool/research-security.md: 0 CRITICAL / 4 WARNING / 10 ADVISORY findings with required mitigations
- docs/plans/proton-migration-tool/research-ux.md: UI patterns, confidence-level visual treatment, gamepad requirements, competitive analysis
- docs/plans/proton-migration-tool/research-practices.md: Reuse inventory, KISS assessment, module boundaries, interface design
- docs/plans/proton-migration-tool/research-business.md: Version suggestion tiers, edge cases, workflow diagrams, domain model
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: Proton discovery, `normalize_alias()` (promote to `pub(crate)`), `resolve_compat_tool_by_name()` — core matching dependencies
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs: `ProtonInstall` struct with `name`, `path`, `normalized_aliases`, `is_official`
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile`, `effective_profile()`, `storage_profile()` — migration roundtrip target
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore::load()`/`save()`/`list()` — profile CRUD
- src/crosshook-native/crates/crosshook-core/src/profile/health.rs: `batch_check_health()`, `HealthIssue` — stale detection pattern
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: `SyncSource` enum — add `AppMigration` variant
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs: `upsert_health_snapshot()` — invalidate after migration
- src/crosshook-native/src-tauri/src/commands/profile.rs: Canonical Tauri command pattern with `State<'_>` injection + `observe_profile_write()`
- src/crosshook-native/src-tauri/src/commands/shared.rs: `sanitize_display_path()` — mandatory for all migration IPC path results
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx: Health Dashboard with `TableToolbar`, `categorizeIssue()`, issue row expansion
- src/crosshook-native/src/components/LauncherPreviewModal.tsx: Modal shell with portal, focus trap, ARIA — base for migration review modal

## Implementation Plan

### Phase 1: Single-Profile Migration

#### Task 1.0: Prerequisite Visibility Changes and Enum Variant Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs

Four small changes that unblock all subsequent tasks:

1. In `steam/proton.rs`, promote `normalize_alias()` from `fn` to `pub(crate) fn`. This function strips non-alphanumeric characters and lowercases — the migration module needs it for family extraction.

2. In `steam/proton.rs`, promote `resolve_compat_tool_by_name()` from `fn` to `pub(crate) fn`. The migration module uses this as a fallback matcher when family-based matching produces no candidates.

3. In `metadata/models.rs`, add `AppMigration` variant to the `SyncSource` enum. **Also add the corresponding arm in `SyncSource::as_str()`** — it uses an exhaustive match, so the compiler will error without it. The arm should return `"app_migration"`.

4. In `metadata/profile_sync.rs`, add `SyncSource::AppMigration` to the exhaustive match in `created_at_for_insert()` (around line 272-277). Add it to the existing `AppWrite | AppRename | ... | Import => None` arm — migration writes do not override `created_at`.

Do NOT add `pub mod migration;` to `profile/mod.rs` yet — wait for Task 1.1 when the module file exists. Run `cargo test -p crosshook-core` to verify compilation.

#### Task 1.1: Backend Version Suggestion Engine Depends on [1.0]

**READ THESE BEFORE TASK**

- docs/plans/proton-migration-tool/feature-spec.md (Data Models + Core Algorithm sections)
- docs/plans/proton-migration-tool/research-technical.md (Algorithm Steps 1-5 with code examples)
- docs/plans/proton-migration-tool/research-business.md (Business Rules BR-1 through BR-10)
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- src/crosshook-native/crates/crosshook-core/src/profile/health.rs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/migration.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs

Create `profile/migration.rs` with the complete suggestion engine. This is pure logic with no Tauri dependency — all functions take `&ProfileStore` and `&[ProtonInstall]` as inputs.

**Data structs** (derive `Serialize, Deserialize, Debug, Clone`):

- `ProtonPathField` enum: `SteamProtonPath`, `RuntimeProtonPath` (serde `rename_all = "snake_case"`)
- `MigrationSuggestion`: `profile_name`, `field`, `old_path`, `new_path`, `old_proton_name`, `new_proton_name`, `confidence: f64`, `proton_family`, `crosses_major_version: bool`
- `UnmatchedProfile`: `profile_name`, `field`, `stale_path`, `stale_proton_name`
- `MigrationScanResult`: `suggestions: Vec<MigrationSuggestion>`, `unmatched: Vec<UnmatchedProfile>`, `profiles_scanned`, `affected_count`, `installed_proton_versions: Vec<ProtonInstallInfo>`, `diagnostics: Vec<String>`
- `ProtonInstallInfo`: `name`, `path: String`, `is_official` (lightweight IPC-safe version of `ProtonInstall`). Note: `path` is the **executable** path (e.g., `.../GE-Proton9-7/proton`), same as `ProtonInstall.path` — not the directory. When extracting the family/version name for display, use `PathBuf::from(path).parent().and_then(|p| p.file_name())`.
- `MigrationOutcome` enum: `Applied`, `AlreadyValid`, `Failed`
- `MigrationApplyResult`: `profile_name`, `field`, `old_path`, `new_path`, `outcome`, `error: Option<String>`
- `ApplyMigrationRequest`: `profile_name`, `field`, `new_path` (deserialize only)
- `BatchMigrationRequest`: `migrations: Vec<ApplyMigrationRequest>`
- `BatchMigrationResult`: `results`, `applied_count`, `failed_count`, `skipped_count`

**Core functions**:

1. `extract_proton_family(name: &str) -> Option<String>` — call `normalize_alias(name)`, then `trim_end_matches(|c: char| c.is_ascii_digit())`. Returns family key like `"geproton"`, `"proton"`, `"protonexperimental"`.

2. `extract_version_segments(name: &str) -> Vec<u32>` — split the **raw** (non-normalized) name on non-digit boundaries, parse each segment as `u32`. `"GE-Proton10-34"` -> `[10, 34]`. Operate on raw name to avoid digit ambiguity from normalization.

3. `find_best_replacement(stale_name: &str, installed: &[ProtonInstall]) -> Option<(ProtonInstall, f64, bool)>` — extract family + version from stale name, iterate installed tools, filter to same family, score by confidence (0.9 same-major newer, 0.75 cross-major newer, 0.7 same-major older, 0.5 cross-major older), sort by version descending, return best match with `(install, confidence, crosses_major)`.

4. `scan_proton_migrations(store: &ProfileStore, steam_root_candidates: &[PathBuf], diagnostics: &mut Vec<String>) -> MigrationScanResult` — **this is the canonical signature** (loads profiles lazily via the store, not pre-loaded). Call `discover_compat_tools()` once, iterate `store.list()`, for each profile call `store.load()`, check effective profile's `steam.proton_path` (if `steam_applaunch`) or `runtime.proton_path` (if `proton_run`) via `path.try_exists()` — only flag as stale on `Ok(false)`. Use `extract_name_from_proton_path()` to get the directory name, call `find_best_replacement()`. Collect suggestions and unmatched.

5. `apply_single_migration(store: &ProfileStore, request: &ApplyMigrationRequest) -> MigrationApplyResult` — load profile, validate replacement path exists and is a file, check if old path already valid (`AlreadyValid`), update the correct field based on `request.field`. **Write using temp+rename (W-1)**: serialize via `toml::to_string_pretty(&profile.storage_profile())`, write to `profile_path.with_extension("toml.tmp")`, then `fs::rename()` to the target path. Do NOT use `store.save()` — it uses non-atomic `fs::write()`. Return result with old/new paths.

6. Helper `extract_name_from_proton_path(path: &str) -> String` — `PathBuf::from(path).parent().and_then(|p| p.file_name()).to_str()`.

Add `pub mod migration;` to `profile/mod.rs`.

**Unit tests** (critical — write these first):

- `extract_proton_family`: GE-Proton -> `"geproton"`, official Proton -> `"proton"`, Experimental -> `"protonexperimental"`, TKG -> `"protontkg..."`
- `extract_version_segments`: `"GE-Proton10-34"` -> `[10, 34]`, `"Proton 9.0-1"` -> `[9, 0, 1]`
- Integer-tuple ordering: `[9, 10] > [9, 9]` (the critical edge case)
- Same-family newer match gets 0.9 confidence
- Cross-major gets `crosses_major_version: true` and 0.75 confidence
- TKG builds excluded from numeric ranking
- "Proton Experimental" is versionless — only matches another Experimental
- No match returns `None` gracefully
- **Round-trip test**: create profile with `steam.proton_path = "/old/proton"`, migrate to `"/new/proton"`, re-load, verify effective path is `"/new/proton"` and on-disk TOML has it in `local_override`

Run `cargo test -p crosshook-core` to verify all tests pass.

#### Task 1.2: Backend Tauri IPC Commands Depends on [1.1]

**READ THESE BEFORE TASK**

- docs/plans/proton-migration-tool/research-security.md (W-1 atomic writes, W-3 consent gate, A-4 sanitize_display_path, A-5 steam path validation)
- src/crosshook-native/src-tauri/src/commands/profile.rs (canonical command pattern)
- src/crosshook-native/src-tauri/src/commands/shared.rs (sanitize_display_path)
- src/crosshook-native/src-tauri/src/commands/steam.rs (default_steam_client_install_path, steam root validation)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/migration.rs

Files to Modify

- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

Create `commands/migration.rs` with two Tauri commands for Phase 1:

1. `check_proton_migrations(steam_client_install_path: Option<String>, store: State<'_, ProfileStore>) -> Result<MigrationScanResult, String>`:
   - Validate `steam_client_install_path` with `candidate.join("steamapps").is_dir()` check; fall back to `default_steam_client_install_path()` if invalid (A-5)
   - Call `discover_steam_root_candidates()` then `scan_proton_migrations()`
   - Apply `sanitize_display_path()` to all `old_path` and `new_path` strings in the result before returning (A-4)
   - This is read-only — no writes, no `MetadataStore` needed

2. `apply_proton_migration(request: ApplyMigrationRequest, store: State<'_, ProfileStore>, metadata_store: State<'_, MetadataStore>) -> Result<MigrationApplyResult, String>`:
   - Re-validate replacement path with `Path::new(&request.new_path).try_exists()` immediately before write (TOCTOU mitigation)
   - Call `apply_single_migration()` from core. **The core function must use temp+rename for all migration writes (W-1)** — do NOT use `store.save()` directly. Instead, serialize via `toml::to_string_pretty(&profile.storage_profile())`, write to `.toml.tmp`, then `fs::rename()`. This applies to both single and batch writes for consistency.
   - On success: call `metadata_store.observe_profile_write()` with the full signature:

     ```rust
     let profile_path = store.base_path.join(format!("{}.toml", request.profile_name));
     if let Err(e) = metadata_store.observe_profile_write(
         &request.profile_name,
         &updated_profile,
         &profile_path,
         SyncSource::AppMigration,
         None, // source_profile_id — not applicable for migration
     ) {
         tracing::warn!(%e, profile = %request.profile_name, "metadata sync after migration failed");
     }
     ```

   - On success: invalidate the health snapshot. The `profile_id` is a UUID from the metadata layer — use the profile row from `observe_profile_write()` or look it up via `metadata_store`. See `commands/health.rs:232` for the pattern of calling `check_profile_health()` then `upsert_health_snapshot()` with the correct `profile_id`.
   - Apply `sanitize_display_path()` to returned paths
   - Return `MigrationApplyResult`

In `commands/mod.rs`, add `pub mod migration;`.

In `lib.rs`, register both commands in `tauri::generate_handler![]`:

```
commands::migration::check_proton_migrations,
commands::migration::apply_proton_migration,
```

Verify compilation with `cargo build --manifest-path src/crosshook-native/Cargo.toml`.

#### Task 1.3: Frontend TypeScript Types and Migration Hook Depends on [1.1]

**READ THESE BEFORE TASK**

- docs/plans/proton-migration-tool/feature-spec.md (TypeScript Types section)
- src/crosshook-native/src/hooks/useProfileHealth.ts (hook pattern, revalidateSingle)
- src/crosshook-native/src/types/index.ts (re-export pattern)

**Instructions**

Files to Create

- src/crosshook-native/src/types/migration.ts
- src/crosshook-native/src/hooks/useProtonMigration.ts

Files to Modify

- src/crosshook-native/src/types/index.ts

Create `types/migration.ts` with direct TypeScript translations of the Rust structs from `feature-spec.md`:

- `ProtonPathField` type: `'steam_proton_path' | 'runtime_proton_path'`
- `MigrationOutcome` type: `'applied' | 'already_valid' | 'failed'`
- `MigrationSuggestion` interface: `profile_name`, `field`, `old_path`, `new_path`, `old_proton_name`, `new_proton_name`, `confidence`, `proton_family`, `crosses_major_version`
- `UnmatchedProfile` interface: `profile_name`, `field`, `stale_path`, `stale_proton_name`
- `ProtonInstallInfo` interface: `name`, `path`, `is_official`
- `MigrationScanResult` interface: `suggestions`, `unmatched`, `profiles_scanned`, `affected_count`, `installed_proton_versions`, `diagnostics`
- `MigrationApplyResult` interface: `profile_name`, `field`, `old_path`, `new_path`, `outcome`, `error`
- `ApplyMigrationRequest` interface: `profile_name`, `field`, `new_path`
- `BatchMigrationResult` interface: `results`, `applied_count`, `failed_count`, `skipped_count`

Create `hooks/useProtonMigration.ts`:

- `scanMigrations(steamClientInstallPath?: string)` — invokes `check_proton_migrations`, manages `loading`/`error`/`result` state
- `applySingleMigration(request: ApplyMigrationRequest)` — invokes `apply_proton_migration`, on success calls `revalidateSingle(request.profile_name)` from the health context
- State: `scanResult: MigrationScanResult | null`, `isScanning: boolean`, `applyResult: MigrationApplyResult | null`, `isApplying: boolean`, `error: string | null`
- No optimistic updates — wait for IPC confirmation before updating state
- Handle TOCTOU error by setting error state with re-scan prompt

In `types/index.ts`, add re-exports for all migration types.

This task can run **in parallel with Task 1.2** since the type contracts are defined in `feature-spec.md` and don't depend on Rust compilation.

#### Task 1.4: Health Dashboard Single-Profile Migration UX Depends on [1.2, 1.3]

**READ THESE BEFORE TASK**

- docs/plans/proton-migration-tool/research-ux.md (User Workflows, Confidence Level Visual Treatment, Error Handling)
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx
- src/crosshook-native/src/styles/variables.css
- src/crosshook-native/src/styles/focus.css

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Integrate single-profile migration actions into the existing Health Dashboard. This modifies only `HealthDashboardPage.tsx` — no new component files for Phase 1.

Changes:

1. **Import the migration hook**: `useProtonMigration` from the new hook file. Wire it into the component.

2. **Per-row migration action**: In the issue row expansion for `missing_proton` category issues, add an "Update Proton" action button. On click:
   - Call `scanMigrations()` to get candidates for this specific profile
   - Display inline suggestion: old path (in `--crosshook-color-danger`) -> new path (in `--crosshook-color-success`) with the Proton version names
   - If `crosses_major_version` is true, show amber warning: "Major version change — WINE prefix may need recreation"
   - If confidence < 0.75 (cross-family), show orange warning: "Different Proton family — verify compatibility"
   - "Use [new_proton_name]" button (descriptive label, not generic "Apply")
   - On click: call `applySingleMigration()` -> on success, show dismissible undo toast (5s timeout) using existing toast pattern if available, or a simple timed notification
   - Health badge auto-refreshes via `revalidateSingle()` called by the hook

3. **No-match state**: If scan returns no suggestion for a profile, show: "No matching Proton installation found. [Browse...]" with a link to the profile editor's Proton path field.

4. **Error display**: If apply fails, show inline error message with the error string from `MigrationApplyResult.error`.

5. **Gamepad/controller accessibility**: All new interactive elements must use `crosshook-focus-ring`, `crosshook-nav-target`, `crosshook-focus-target` CSS classes. Button minimum height: `var(--crosshook-touch-target-min)`.

6. **Path display**: Show sanitized paths (home dir replaced with `~` — already done server-side by `sanitize_display_path()`).

7. **Language**: Use empathetic, recovery-focused messaging per research-ux.md. "GE-Proton 9-4 is no longer installed" not "Error: path invalid".

Do NOT add the batch "Fix All" toolbar button yet — that's Phase 2.

### Phase 2: Batch Migration

#### Task 2.1: Backend Batch Migration Command Depends on [1.2, 1.4]

**READ THESE BEFORE TASK**

- docs/plans/proton-migration-tool/research-security.md (W-1 atomic writes for batch, W-4 pre-flight validation)
- docs/plans/proton-migration-tool/research-business.md (BR-6 batch pre-flight, BR-9 cross-family exclusion)
- src/crosshook-native/src-tauri/src/commands/migration.rs (existing commands from Task 1.2)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/migration.rs
- src/crosshook-native/src-tauri/src/lib.rs

Add `apply_batch_migration` command to `commands/migration.rs`:

`apply_batch_migration(request: BatchMigrationRequest, store: State<'_, ProfileStore>, metadata_store: State<'_, MetadataStore>) -> Result<BatchMigrationResult, String>`:

1. **Pre-flight validation pass (W-4 — non-negotiable)**: Before any writes, iterate all requests:
   - Verify each replacement path exists via `Path::new(&m.new_path).try_exists() == Ok(true)`
   - Load each profile and verify serialization succeeds
   - If ANY pre-flight check fails, return immediately with zero writes and the failure details

2. **Atomic write path (W-1)**: For batch writes, use temp+rename pattern:

   ```rust
   let tmp = profile_path.with_extension("toml.tmp");
   fs::write(&tmp, toml::to_string_pretty(&storage_profile)?)?;
   fs::rename(&tmp, &profile_path)?;
   ```

   Do NOT use `store.save()` for batch writes — use the atomic pattern directly.

3. **Per-profile error isolation**: After pre-flight passes, iterate and write each profile. One failure does not abort remaining writes. Collect all results.

4. **Post-write for each success**: `observe_profile_write(SyncSource::AppMigration)` + `upsert_health_snapshot()` (both fail-soft).

5. **Return**: `BatchMigrationResult` with per-profile outcomes, counts.

Register `commands::migration::apply_batch_migration` in `lib.rs` `invoke_handler`.

#### Task 2.2: Frontend Batch Migration Review Modal and Toolbar Depends on [2.1]

**READ THESE BEFORE TASK**

- docs/plans/proton-migration-tool/research-ux.md (Migration Review Modal design, Batch flow, Gamepad navigation)
- src/crosshook-native/src/components/LauncherPreviewModal.tsx (modal shell to copy)
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx (for expandable sections)
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/MigrationReviewModal.tsx

Files to Modify

- src/crosshook-native/src/hooks/useProtonMigration.ts
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

1. **MigrationReviewModal.tsx**: Copy the `LauncherPreviewModal` shell verbatim (portal host, focus trap, Tab cycling, Escape handler, backdrop click, `inert` background management, `aria-modal`, focus restore). Replace the body with:
   - **Header**: "Fix Proton Paths" with affected count
   - **Safe section** (pre-checked): same-family same-major suggestions with green indicator
   - **Needs review section** (collapsed `CollapsibleSection`, unchecked): cross-major and cross-family suggestions with amber/orange warnings
   - **No suggestion section**: informational rows with no checkboxes
   - Per-row: checkbox, profile name, affected field ("Steam Proton" / "Runtime Proton"), old path (danger color), new path (success color), confidence badge
   - "Show full path" expand trigger per row using `CollapsibleSection`
   - Select All toggle (excludes cross-family rows)
   - **Footer**: "Update N Profile(s)" confirm button (N updates as checkboxes toggle), Cancel button (ghost style)
   - Progress bar for >=3 profiles during apply phase
   - Post-migration summary: "X updated, Y failed" with per-failure detail
   - Controller prompts: "A: Toggle B: Cancel Start: Confirm"
   - All elements use `crosshook-focus-ring`, `crosshook-nav-target` CSS classes
   - Tab order: Select All -> checkboxes -> Cancel -> Update N Profiles

2. **useProtonMigration.ts**: Add `applyBatchMigration(requests: ApplyMigrationRequest[])` — invokes `apply_batch_migration`, on completion calls `batchValidate()` from health context to refresh all badges.

3. **HealthDashboardPage.tsx**: Add "Fix Proton Paths (N)" button to the `TableToolbar` component (file-local — modify in place). Only visible when `missing_proton` category count >= 2. On click: call `scanMigrations()` -> open `MigrationReviewModal` with results. After modal closes with successful apply, health badges auto-refresh via `batchValidate()`.

## Advice

- **Task 1.0 is a 10-minute warm-up that unblocks everything.** Assign it first to whichever implementor is available — it's two visibility promotions and one enum variant across two files.

- **The `SyncSource::as_str()` method uses an exhaustive match.** If you add `AppMigration` to the enum but forget the `as_str()` arm, compilation fails. Add both together in Task 1.0.

- **Integer-tuple version comparison is the highest-risk algorithm.** `"9-10"` must sort after `"9-9"` — lexicographic comparison gets this wrong. Write the `extract_version_segments` test first, then implement. If you see `Vec<u32>` comparison producing wrong results, you're likely normalizing the name before digit extraction (don't — operate on the raw name).

- **`ProfileStore::load()` returns the effective profile with `local_override` already merged.** After migration updates `steam.proton_path` on this effective profile, `save()` calls `storage_profile()` which moves the path back to `local_override` on disk. You do NOT need to manually touch `local_override` fields. The `storage_profile_roundtrip_is_idempotent` test at `models.rs:492` proves this.

- **`ProtonInstall.path` is the executable, not the directory.** It ends in `.../proton`. When extracting the family name for matching, call `Path::parent()` then `.file_name()` to get the directory name (e.g., `"GE-Proton9-7"`).

- **Cross-family suggestions must NEVER appear in batch default selections.** The Heroic Games Launcher v2.18.0 incident showed that silently substituting Proton families causes immediate user backlash. Cross-family rows must be unchecked and in a collapsed "Needs Manual Review" section.

- **`sanitize_display_path()` is mandatory on all IPC path results.** Import from `commands/shared.rs` and apply to every `old_path` and `new_path` string before returning from Tauri commands. This replaces `$HOME` with `~` for consistent display.

- **The migration review modal must use `LauncherPreviewModal` as its shell, NOT `ProfileReviewModal`.** `ProfileReviewModal` has a summary-item layout with no checkboxes or table — wrong for this use case. `LauncherPreviewModal` has the full accessibility infrastructure (portal, focus trap, Tab cycling, Escape, inert background).

- **Phase 2's batch pre-flight validation is non-negotiable (security W-4).** Serialize all target profiles and verify all replacement paths before the first write. If any pre-flight check fails, abort with zero writes. This is simpler than backup/rollback and prevents partial-success states.

- **Tasks 1.2 and 1.3 can run in parallel.** The TypeScript type contracts are fully specified in `feature-spec.md` — a frontend implementor does not need to wait for Rust compilation. The hook can be written against the invoke contract and verified once 1.2 merges.

- **Do not extract a `MigrationActionRow` component in Phase 1.** The inline Health Dashboard changes for single-profile migration are ~50-80 lines in `HealthDashboardPage.tsx`. The `TableToolbar` is already file-local. Extract to a separate component only in Phase 2 when the modal and batch toolbar justify the abstraction boundary.
