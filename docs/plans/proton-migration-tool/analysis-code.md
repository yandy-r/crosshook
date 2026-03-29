# Proton Migration Tool — Code Analysis

Comprehensive analysis of the source files required to implement the proton-migration-tool feature. Covers patterns, integration points, naming conventions, and implementation gotchas extracted directly from the codebase.

---

## Executive Summary

The migration tool fits cleanly into three existing patterns: the **Proton discovery pipeline** (`steam/proton.rs`), the **profile load/save roundtrip** (`profile/toml_store.rs`), and the **Tauri IPC command + metadata sync** pattern (`commands/profile.rs`). The only non-trivial code changes are: (1) promoting two private functions in `proton.rs` to `pub(crate)`, (2) adding an `AppMigration` variant to `SyncSource`, and (3) a new `profile/migration.rs` module with three Tauri commands. The UI integrates into the existing Health Dashboard without a new page.

> **Module placement:** The new core module is `profile/migration.rs`, **not** `steam/migration.rs`. Placing it under `profile/` matches where the write side of the operation lives and is the consensus per the practices research doc.

---

## Existing Code Structure

### Rust workspace layout

```
crates/crosshook-core/src/
  steam/
    mod.rs          → pub re-exports; promote normalize_alias + resolve_compat_tool_by_name
    models.rs       → ProtonInstall struct (private module, pub re-exports)
    proton.rs       → discover_compat_tools(), normalize_alias() [currently private → pub(crate)]
    discovery.rs    → discover_steam_root_candidates()
  profile/
    models.rs       → GameProfile, LocalOverrideSection, effective_profile(), storage_profile()
    toml_store.rs   → ProfileStore::load()/save()/list() [save() is non-atomic — see Gotchas]
    health.rs       → check_profile_health(), batch_check_health(), HealthIssue
    migration.rs    → NEW: scan_proton_migrations(), apply_proton_migration() (module placement here)
  metadata/
    models.rs       → SyncSource enum (add AppMigration here)
    mod.rs          → MetadataStore impl, observe_profile_write()
    health_store.rs → upsert_health_snapshot()

src-tauri/src/
  lib.rs            → invoke_handler! registration (add migration commands here)
  commands/
    mod.rs          → module declarations (add migration.rs)
    migration.rs    → NEW: 3 Tauri commands wrapping profile::migration
    profile.rs      → canonical command pattern to copy
    health.rs       → batch + single health IPC, build_enriched_health_summary()
    steam.rs        → list_proton_installs() (reuse pattern for migration scan)
    shared.rs       → sanitize_display_path() (apply to all path IPC results)
```

---

## Implementation Patterns

### 1. Tauri IPC Command (canonical pattern)

From `src-tauri/src/commands/profile.rs:99-116`:

```rust
#[tauri::command]
pub fn profile_save(
    name: String,
    data: GameProfile,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String> {
    store.save(&name, &data).map_err(map_error)?;

    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) =
        metadata_store.observe_profile_write(&name, &data, &profile_path, SyncSource::AppWrite, None)
    {
        tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save failed");
    }

    Ok(())
}
```

**Pattern rules:**

- Function name is `snake_case`, matches the frontend `invoke()` call string
- Parameters use named args (not positional) + `State<'_, T>` injections
- `Result<T, String>` — errors are stringified with `.map_err(|e| e.to_string())`
- Metadata sync is **fail-soft**: log warning, never return error to frontend
- `SyncSource` variant identifies the write source in the audit trail

### 2. Profile Load → Mutate → Save (migration roundtrip)

From `profile/toml_store.rs:100-118`:

```rust
// load() returns effective_profile() with local_override cleared
pub fn load(&self, name: &str) -> Result<GameProfile, ProfileStoreError> {
    let content = fs::read_to_string(&path)?;
    let profile: GameProfile = toml::from_str(&content)?;
    let mut effective = profile.effective_profile();
    effective.local_override = LocalOverrideSection::default();
    Ok(effective)
}

// save() calls storage_profile() which pushes all paths → local_override
pub fn save(&self, name: &str, profile: &GameProfile) -> Result<(), ProfileStoreError> {
    let storage_profile = profile.storage_profile();
    fs::write(path, toml::to_string_pretty(&storage_profile)?)?;
    Ok(())
}
```

**Migration uses exactly this cycle.** Load the profile (get effective flat paths), mutate `steam.proton_path` or `runtime.proton_path`, call `save()`. The `local_override` layer is handled transparently — no special handling needed.

The two proton path fields to target:

- `profile.steam.proton_path` — used when `launch.method == "steam_applaunch"`
- `profile.runtime.proton_path` — used when `launch.method == "proton_run"`

### 3. Proton Discovery Pipeline

From `steam/proton.rs:24-34` and `commands/steam.rs:34-49`:

```rust
// Step 1: get steam root candidates
let steam_root_candidates = discover_steam_root_candidates(configured_path, &mut diagnostics);

// Step 2: discover all installed compat tools
let installs = discover_compat_tools(&steam_root_candidates, &mut diagnostics);
// → Vec<ProtonInstall> { name, path (to `proton` executable), is_official, aliases, normalized_aliases }
```

**Important:** `ProtonInstall.path` is the path to the `proton` **executable**, not the directory (e.g. `/home/user/.local/share/Steam/steamapps/common/Proton 9.0/proton`). The health check compares the profile's `proton_path` string against this.

### 4. normalize_alias + Matching Logic

From `steam/proton.rs:411-456` — **currently private, must be promoted**:

```rust
fn normalize_alias(alias: &str) -> Option<String> {
    // strips all non-alphanumeric, lowercases
    // "GE Proton 9.7" → Some("geproton97")
    // "   " → None
}

fn resolve_compat_tool_by_name(requested: &str, installed: &[ProtonInstall]) -> Vec<&ProtonInstall> {
    // Tier 1: exact alias match (case-insensitive)
    // Tier 2: normalized alias match (alphanumeric-only comparison)
    // Tier 3: heuristic (substring + version digit extraction for "proton*" names)
}
```

For migration, the key operation is: given a stale `proton_path`, extract the directory name (last path component before `/proton`), then run it through this matching pipeline against all current installs to find the best replacement candidate.

### 5. Batch Health Pattern

From `profile/health.rs:451-526`:

```rust
pub fn batch_check_health(store: &ProfileStore) -> HealthCheckSummary {
    let names = match store.list() { ... };
    let mut profiles = Vec::with_capacity(names.len());
    for name in &names {
        let report = match store.load(name) {
            Ok(profile) => check_profile_health(name, &profile),
            Err(err) => ProfileHealthReport { /* Broken sentinel */ },
        };
        profiles.push(report);
    }
    // count healthy/stale/broken
}
```

**Migration batch scan follows this same best-effort pattern.** Iterate all profiles, never abort on per-profile errors, collect results.

### 6. Health Issue Field Names

From `profile/health.rs:376-400` and `HealthDashboardPage.tsx:39-48`:

```rust
// Rust side — fields to check for migration
"steam.proton_path"   // steam_applaunch method
"runtime.proton_path" // proton_run method
```

```tsx
// TypeScript side — categorizeIssue() mapping
if (field === 'steam.proton_path' || field === 'runtime.proton_path') return 'missing_proton';
```

The migration toolbar action in the Health Dashboard triggers when `missing_proton` category count > 0.

### 7. Modal Shell Pattern

From `LauncherPreviewModal.tsx:51-303`:

```tsx
// Key structural elements to copy:
const portalHostRef = useRef<HTMLElement | null>(null);
const surfaceRef = useRef<HTMLDivElement | null>(null);
const previouslyFocusedRef = useRef<HTMLElement | null>(null);
const hiddenNodesRef = useRef<Array<...>>([]);
const titleId = useId();
const [isMounted, setIsMounted] = useState(false);

// Effect 1: create/destroy portal host in document.body
// Effect 2: focus management + background inert + scroll lock (gated on isMounted)
// handleKeyDown: Escape closes, Tab cycles through getFocusableElements(surfaceRef)
// handleBackdropMouseDown: close on backdrop click (not surface click)
// Render: createPortal(<div role="presentation">...<div role="dialog" aria-modal aria-labelledby>
```

New migration review modal copies this shell verbatim, replacing the `<body>` content with the candidate list.

### 8. SyncSource — Adding AppMigration

From `metadata/models.rs:76-98`:

```rust
pub enum SyncSource {
    AppWrite,
    AppRename,
    AppDuplicate,
    AppDelete,
    FilesystemScan,
    Import,
    InitialCensus,
    // ADD:
    AppMigration,
}

impl SyncSource {
    pub fn as_str(self) -> &'static str {
        match self {
            // ... existing arms ...
            Self::AppMigration => "app_migration",  // ADD this arm
        }
    }
}
```

`as_str()` is exhaustive-match; Rust will warn if the new variant is missing here.

### 9. Health Snapshot Invalidation

From `commands/health.rs:223-245`:

```rust
// After migration write, invalidate the snapshot so the UI shows fresh status
if let Some(ref profile_id) = metadata.profile_id {
    if let Err(error) = metadata_store.upsert_health_snapshot(
        profile_id, status_str, issue_count, &checked_at
    ) {
        tracing::warn!(%error, profile_id, "failed to persist health snapshot");
    }
}
```

Apply migration → call `check_profile_health()` on updated profile → call `upsert_health_snapshot()` with new status. Keeps cached badges accurate without requiring a full re-scan.

### 10. Command Registration

`src-tauri/src/lib.rs:113-172` — `tauri::generate_handler!` macro. Add migration commands:

```rust
commands::steam::check_proton_migrations,
commands::steam::apply_proton_migration,
commands::steam::apply_batch_migration,
```

Or add a new `commands/migration.rs` and add `pub mod migration;` to `commands/mod.rs`.

### 11. sanitize_display_path — Required for IPC Paths

From `commands/shared.rs:20-33`:

```rust
pub fn sanitize_display_path(path: &str) -> String {
    // HOME → "~" prefix
}
```

**All path strings returned over IPC must pass through this.** The health command already wraps with `sanitize_issues()` before return. Migration command must do the same for `current_path`, `suggested_path` fields in the IPC response struct.

---

## Integration Points

### Files to Create

| File                                             | Purpose                                                                |
| ------------------------------------------------ | ---------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/migration.rs` | Core migration logic: scan stale paths, match candidates, apply writes |
| `src-tauri/src/commands/migration.rs`            | Three Tauri commands: check/apply/batch-apply                          |
| `src/types/migration.ts`                         | TypeScript types for migration IPC structs                             |
| `src/components/MigrationReviewModal.tsx`        | Migration review modal (copy LauncherPreviewModal shell)               |
| `src/hooks/useMigrationState.ts`                 | Hook wrapping migration IPC state                                      |

### Files to Modify

| File                                           | Change                                                                                                  |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `steam/proton.rs`                              | Promote `normalize_alias` and `resolve_compat_tool_by_name` to `pub(crate)`                             |
| `profile/mod.rs` (or `profile/lib.rs`)         | Add `pub mod migration;`                                                                                |
| `metadata/models.rs`                           | Add `AppMigration` variant to `SyncSource` + `as_str()` arm                                             |
| `commands/mod.rs`                              | Add `pub mod migration;`                                                                                |
| `src-tauri/src/lib.rs`                         | Register three new commands in `invoke_handler!`                                                        |
| `src/components/pages/HealthDashboardPage.tsx` | Add migration toolbar button to `TableToolbar` (file-local, not exported) + per-row "Fix Proton" action |
| `src/types/index.ts`                           | Re-export migration types                                                                               |

---

## Code Conventions

### Rust

- Function names: `snake_case` — command names must match the `invoke('...')` call string in TypeScript
- Error stringification: `map_err(|e| e.to_string())` — never return raw `ProfileStoreError` over IPC
- Diagnostics: `Vec<String>` passed `&mut` through discovery functions (not a `Result` error channel)
- Fail-soft metadata ops: `if let Err(e) = ... { tracing::warn!(...) }` — never propagate
- Test pattern: `tempfile::tempdir()` for isolation, `ProfileStore::with_base_path(tmp.path().join(...))` to bypass home dir
- `#[cfg(test)] mod tests { use super::*; }` at end of file

### TypeScript / React

- Hook naming: `useMigrationState.ts` → `use` prefix, camelCase
- IPC invocation: `invoke<ReturnType>('command_name', { arg1, arg2 })` — string matches Rust function name
- Component CSS: `crosshook-*` BEM prefix, e.g. `crosshook-migration-modal__surface`
- Modal portals: always `createPortal(jsx, portalHostRef.current)` — never inline in the tree
- IPC types: define in `src/types/migration.ts`, re-export from `src/types/index.ts`

---

## Dependencies and Services

### Rust crate dependencies (already present)

- `chrono` — `Utc::now().to_rfc3339()` for timestamps
- `serde` with `Serialize, Deserialize` — all IPC-crossing types
- `rusqlite` (via MetadataStore) — health snapshot invalidation
- `tempfile` (dev-dependency) — test isolation
- `tracing` — structured logging

### Tauri State injections for migration commands

```rust
store: State<'_, ProfileStore>,
metadata_store: State<'_, MetadataStore>,
```

Both are `.manage()`-ed in `lib.rs`. No new state to register.

### Function call chain for migration scan

```
discover_steam_root_candidates(configured_path, &mut diagnostics)
  → Vec<PathBuf>
  → discover_compat_tools(&candidates, &mut diagnostics)
  → Vec<ProtonInstall>
  → [for each stale profile path]:
      normalize_alias(stale_dir_name)  // pub(crate) after promotion
      → compare against ProtonInstall.normalized_aliases
```

---

## Gotchas and Warnings

- **`normalize_alias` is private.** It lives in `steam/proton.rs` as `fn normalize_alias(alias: &str) -> Option<String>`. The migration module needs it. Change to `pub(crate)`. Same for `resolve_compat_tool_by_name` (also private). These are the only visibility promotions needed.

- **`ProtonInstall.path` is the executable, not the directory.** The path ends in `.../proton` (the shell script). When comparing a profile's `steam.proton_path` to an installed tool, you compare the full path string. When extracting the "family name" for matching, do `Path::parent()` then `.file_name()`.

- **`ProfileStore::load()` strips `local_override`.** The returned `GameProfile` has all paths in their top-level fields (e.g., `steam.proton_path` is populated, `local_override.steam.proton_path` is empty). This is the correct surface for migration — read and write top-level fields directly.

- **`storage_profile()` always pushes paths to `local_override`.** After mutation and `save()`, the TOML file on disk will have the new path under `[local_override.steam]`. This is the intended behavior (machine-local paths belong in local_override). No special override handling is needed.

- **`SyncSource::as_str()` is exhaustive match.** Adding `AppMigration` to the enum without adding the `as_str()` arm causes a compile error. Both must be added together.

- **Health snapshot must be invalidated after migration.** The `health_snapshots` table is populated by `batch_validate_profiles` and read back by `get_cached_health_snapshots` for instant badge display. After applying migration, call `metadata_store.upsert_health_snapshot()` for each migrated profile so the cached status reflects the fix.

- **`diagnostics` vec in discovery is not an error channel.** `discover_compat_tools()` never returns `Err(...)` — it pushes informational/warning strings into `&mut Vec<String>`. Log these with `tracing::debug!` in the command handler, don't expose them as command errors.

- **`commands/mod.rs` exists and must be updated.** The commands module is not auto-discovered. Adding `migration.rs` under `commands/` requires `pub mod migration;` in `commands/mod.rs`.

- **`HealthDashboardPage.tsx` is large (~600 lines).** The migration integration (toolbar button + per-row "Fix Proton" button) should be additive — new props to `TableToolbar` and a new conditional action in the issue expansion row. Do not restructure existing state.

- **No async commands in the existing pattern.** The `profile_save`, `batch_validate_profiles` commands are synchronous. `auto_populate_steam` uses `tauri::async_runtime::spawn_blocking`. Migration commands should be synchronous (filesystem ops + TOML writes are fast) unless scan across many profiles justifies async.

- **Path traversal is blocked by `validate_name()` in `ProfileStore`.** Profile names reject `/`, `\`, `:`, path-traversal strings. Migration receives profile names from `store.list()` — already validated. Suggested Proton paths come from `discover_compat_tools()` which reads only from known filesystem roots.

- **`ProfileStore::save()` is non-atomic** (`toml_store.rs:117`). It calls `fs::write(path, ...)` which truncates then writes — a crash mid-write leaves a zero-length or partial TOML file. For the single-profile `apply_proton_migration` case this is the same risk as any other profile save. For **batch migration** (`apply_batch_migration`), use an atomic write pattern: serialize to `profile_name.toml.tmp`, then `fs::rename()` to the final path. `fs::rename()` is atomic on Linux (same filesystem). This is the only place in the codebase where a batch write makes this matter.

- **Version comparison MUST use integer-tuple parsing, not string ordering.** Proton version names like `"GE-Proton9-10"` vs `"GE-Proton9-9"` sort incorrectly lexicographically (`"9-10" < "9-9"` because `'1' < '9'`). When ranking candidates or picking the "latest" of a family, parse version segments as `(u32, u32)` tuples. Example: extract digits from the normalized form `"geproton910"` → parse as `(9, 10)` for numeric comparison.

- **`TableToolbar` is file-local in `HealthDashboardPage.tsx`.** It is NOT exported from the file — it's a module-private React component defined inline. New migration props must be added directly to the `TableToolbar` function signature and call site within the same file. Do not attempt to import it from elsewhere.

---

## Task-Specific Guidance

### Task: `profile/migration.rs` (backend scan + apply logic)

Module lives under `crates/crosshook-core/src/profile/migration.rs`. It imports from `crate::steam::proton::{discover_compat_tools, normalize_alias, resolve_compat_tool_by_name}` (after visibility promotions) and `crate::steam::discovery::discover_steam_root_candidates`.

```rust
// Skeleton
pub struct ProtonMigrationCandidate {
    pub profile_name: String,
    pub field: String,          // "steam.proton_path" | "runtime.proton_path"
    pub current_path: String,   // stale path
    pub suggested_path: String, // best match from discover_compat_tools
    pub confidence: f32,        // 0.0-1.0 based on match tier
    pub match_reason: String,   // "exact_alias" | "normalized_alias" | "heuristic"
}

pub fn scan_proton_migrations(store: &ProfileStore, steam_root: &str) -> Vec<ProtonMigrationCandidate> {
    // 1. discover_steam_root_candidates(steam_root, &mut diag)
    // 2. discover_compat_tools(&candidates, &mut diag)
    // 3. store.list() → iterate profiles
    // 4. For each profile: if proton_path is set AND path does not exist on disk:
    //      extract dir name from stale path (Path::parent().file_name())
    //      normalize_alias(dir_name) → compare against ProtonInstall.normalized_aliases
    // 5. Return candidates (best match only, skip if ambiguous)
    // NOTE: Version ranking uses integer-tuple parsing, not lexicographic order
}

pub fn apply_proton_migration(store: &ProfileStore, profile_name: &str, field: &str, new_path: &str)
    -> Result<GameProfile, ProfileStoreError>
{
    let mut profile = store.load(profile_name)?;
    match field {
        "steam.proton_path" => profile.steam.proton_path = new_path.to_string(),
        "runtime.proton_path" => profile.runtime.proton_path = new_path.to_string(),
        _ => return Err(ProfileStoreError::InvalidName(field.to_string())),
    }
    store.save(profile_name, &profile)?;  // single-profile: atomic enough
    Ok(profile)
}

// For batch: write to .toml.tmp then fs::rename() — DO NOT call store.save() in a loop
pub fn apply_batch_migration(store: &ProfileStore, candidates: &[(&str, &str, &str)])
    -> Vec<Result<GameProfile, ProfileStoreError>>
{
    candidates.iter().map(|(name, field, new_path)| {
        let mut profile = store.load(name)?;
        // mutate field...
        // atomic write: serialize → write to name.toml.tmp → fs::rename to name.toml
        Ok(profile)
    }).collect()
}
```

### Task: Three Tauri commands (in `commands/migration.rs`)

Follow `profile_save` pattern exactly:

1. Call core logic
2. `.map_err(|e| e.to_string())`?
3. Post-save: `metadata_store.observe_profile_write(..., SyncSource::AppMigration, None)` fail-soft
4. Invalidate health snapshot: `check_profile_health(name, &updated_profile)` → `upsert_health_snapshot()` fail-soft
5. Return sanitized result (`sanitize_display_path` on all path fields)

### Task: Frontend — HealthDashboardPage migration button

The `TableToolbar` component (lines 110-183 of HealthDashboardPage.tsx) needs a new "Fix Proton" button that appears when `missing_proton` count > 0. Pass down: `migrationCandidateCount`, `onRunMigration` props. Inside `onRunMigration`, invoke `check_proton_migrations` to get candidates, then show `MigrationReviewModal`.

### Task: `MigrationReviewModal.tsx`

Copy `LauncherPreviewModal.tsx` structure wholesale:

- Same portal host, focus trap, Tab cycle, Escape close, backdrop click close
- Replace body: a list of `ProtonMigrationCandidate` rows with checkboxes
- Footer: "Apply Selected" → `invoke('apply_batch_migration', { selections })` → close + trigger health recheck

### Task: TypeScript types (`src/types/migration.ts`)

```typescript
export interface ProtonMigrationCandidate {
  profile_name: string;
  field: 'steam.proton_path' | 'runtime.proton_path';
  current_path: string;
  suggested_path: string;
  confidence: number;
  match_reason: 'exact_alias' | 'normalized_alias' | 'heuristic';
}

export interface MigrationApplyResult {
  profile_name: string;
  applied: boolean;
  error?: string;
}
```
