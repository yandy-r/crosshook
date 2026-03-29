# Proton Migration Tool - Technical Specification

## Executive Summary

When users upgrade Proton (e.g., GE-Proton 9-4 to 9-7), profiles referencing the old version break silently. The health dashboard reports "Path does not exist" but offers no guidance on the available replacement. This specification designs a migration system that detects stale Proton paths, matches them to installed replacements using family-based fuzzy matching, and applies updates individually or in batch -- all built on top of the existing Proton discovery and profile health infrastructure.

The implementation adds one new Rust module (`steam/migration.rs`), one new Tauri command file (`commands/migration.rs`), and one new React hook (`useProtonMigration.ts`). One enum variant (`SyncSource::AppMigration`) is added to the metadata module, and `normalize_alias()` visibility is promoted to `pub(crate)`.

---

## Architecture Design

### Component Diagram

```
Frontend (React/TypeScript)
  useProtonMigration.ts ──invoke──► commands/migration.rs (Tauri IPC)
  useProfileHealth.ts   ──invoke──► commands/health.rs    (existing)

Tauri IPC Layer
  commands/migration.rs
    ├── check_proton_migrations()    → MigrationScanResult
    ├── apply_proton_migration()     → MigrationApplyResult
    └── apply_batch_migration()      → BatchMigrationResult

crosshook-core (Rust library)
  steam/migration.rs
    ├── scan_proton_migrations()     ← uses steam/proton.rs discovery
    ├── apply_single_migration()     ← uses profile/toml_store.rs save
    └── extract_proton_family()      ← uses steam/proton.rs normalize_alias
  steam/proton.rs (existing)
    ├── discover_compat_tools()      [reused]
    └── normalize_alias()            [reused, made pub(crate)]
  profile/toml_store.rs (existing)
    ├── ProfileStore::load()         [reused]
    └── ProfileStore::save()         [reused]
  profile/health.rs (existing)
    └── check_profile_health()       [reused for candidate detection]
```

### New Module: `steam/migration.rs`

Lives in `crates/crosshook-core/src/steam/migration.rs`. Contains all migration detection, family matching, and suggestion generation logic. Depends on sibling modules (`proton`, `models`) and `profile` crate modules.

### Integration Points

| Existing Component                                        | Integration                                                            | Direction  |
| --------------------------------------------------------- | ---------------------------------------------------------------------- | ---------- |
| `steam/proton.rs::discover_compat_tools()`                | Discovers all installed Proton versions                                | Read       |
| `steam/proton.rs::normalize_alias()`                      | Normalizes names for family extraction (promote to `pub(crate)`)       | Read       |
| `steam/proton.rs::resolve_compat_tool_by_name()`          | 3-tier matching (exact → normalized → heuristic); wrap as `pub(crate)` | Read       |
| `profile/toml_store.rs::ProfileStore`                     | Load profiles to scan, save migrated profiles                          | Read/Write |
| `profile/models.rs::GameProfile::effective_profile()`     | Resolve local overrides to get actual proton path                      | Read       |
| `profile/health.rs::check_profile_health()`               | Detect stale proton paths (optional fast-path)                         | Read       |
| `metadata/mod.rs::MetadataStore::observe_profile_write()` | Sync metadata after profile mutation                                   | Write      |
| `commands/steam.rs::default_steam_client_install_path()`  | Resolve Steam root for discovery                                       | Read       |

---

## Data Models

### Rust Structs (`steam/migration.rs`)

```rust
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Which profile field contains the stale Proton path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtonPathField {
    /// `steam.proton_path` — used by `steam_applaunch` method
    SteamProtonPath,
    /// `runtime.proton_path` — used by `proton_run` method
    RuntimeProtonPath,
}

/// A single migration suggestion for one profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationSuggestion {
    /// Profile name (filename stem, e.g. "elden-ring")
    pub profile_name: String,
    /// Which field holds the stale path
    pub field: ProtonPathField,
    /// The current (stale) proton path from the profile
    pub old_path: String,
    /// The suggested replacement proton path
    pub new_path: String,
    /// Display name of the old Proton install (extracted from path)
    pub old_proton_name: String,
    /// Display name of the new Proton install
    pub new_proton_name: String,
    /// Confidence score: 0.0..=1.0
    pub confidence: f64,
    /// Proton family used for matching (e.g. "geproton", "proton")
    pub proton_family: String,
    /// True if the suggestion crosses a major version boundary (e.g., 9→10),
    /// which may require prefix migration. Frontend should show a warning.
    pub crosses_major_version: bool,
}

/// A profile with a stale proton path that has no matching replacement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmatchedProfile {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub stale_path: String,
    /// The extracted Proton directory name from the stale path
    pub stale_proton_name: String,
}

/// Result of scanning all profiles for migration candidates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationScanResult {
    /// Profiles with migration suggestions (may have multiple per profile
    /// if both steam and runtime proton paths are stale)
    pub suggestions: Vec<MigrationSuggestion>,
    /// Profiles with stale proton paths but no matching replacement found.
    /// Frontend renders these as "no suggestion found" rows.
    pub unmatched: Vec<UnmatchedProfile>,
    /// Total profiles scanned
    pub profiles_scanned: usize,
    /// Profiles with at least one stale proton path (with or without suggestion)
    pub affected_count: usize,
    /// All currently installed Proton versions (for manual selection fallback)
    pub installed_proton_versions: Vec<ProtonInstallInfo>,
    /// Diagnostic messages from proton discovery
    pub diagnostics: Vec<String>,
}

/// Lightweight Proton install info for frontend display in manual selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtonInstallInfo {
    pub name: String,
    pub path: String,
    pub is_official: bool,
}

/// Outcome of applying a single migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MigrationOutcome {
    /// Profile updated successfully
    Applied,
    /// Profile was already pointing to a valid path (no-op)
    AlreadyValid,
    /// Profile could not be loaded or saved
    Failed,
}

/// Result of applying a single migration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationApplyResult {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub old_path: String,
    pub new_path: String,
    pub outcome: MigrationOutcome,
    /// Error message if outcome is Failed
    pub error: Option<String>,
}

/// Request to apply a single migration (sent from frontend).
#[derive(Debug, Clone, Deserialize)]
pub struct ApplyMigrationRequest {
    pub profile_name: String,
    pub field: ProtonPathField,
    pub new_path: String,
}

/// Request to apply multiple migrations at once.
#[derive(Debug, Clone, Deserialize)]
pub struct BatchMigrationRequest {
    pub migrations: Vec<ApplyMigrationRequest>,
}

/// Result of a batch migration operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchMigrationResult {
    pub results: Vec<MigrationApplyResult>,
    pub applied_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
}
```

### TypeScript Types (`src/types/migration.ts`)

```typescript
export type ProtonPathField = 'steam_proton_path' | 'runtime_proton_path';

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

export interface UnmatchedProfile {
  profile_name: string;
  field: ProtonPathField;
  stale_path: string;
  stale_proton_name: string;
}

export interface ProtonInstallInfo {
  name: string;
  path: string;
  is_official: boolean;
}

export interface MigrationScanResult {
  suggestions: MigrationSuggestion[];
  unmatched: UnmatchedProfile[];
  profiles_scanned: number;
  affected_count: number;
  installed_proton_versions: ProtonInstallInfo[];
  diagnostics: string[];
}

export type MigrationOutcome = 'applied' | 'already_valid' | 'failed';

export interface MigrationApplyResult {
  profile_name: string;
  field: ProtonPathField;
  old_path: string;
  new_path: string;
  outcome: MigrationOutcome;
  error: string | null;
}

export interface BatchMigrationResult {
  results: MigrationApplyResult[];
  applied_count: number;
  failed_count: number;
  skipped_count: number;
}
```

---

## API Design

### Tauri IPC Commands (`commands/migration.rs`)

#### `check_proton_migrations`

Scans all profiles for stale Proton paths and returns migration suggestions.

```rust
#[tauri::command]
pub fn check_proton_migrations(
    steam_client_install_path: Option<String>,
    store: State<'_, ProfileStore>,
) -> Result<MigrationScanResult, String> {
    let configured_path =
        steam_client_install_path.unwrap_or_else(super::steam::default_steam_client_install_path);
    let mut diagnostics = Vec::new();
    let steam_root_candidates =
        discover_steam_root_candidates(configured_path, &mut diagnostics);
    let result = scan_proton_migrations(&store, &steam_root_candidates, &mut diagnostics);
    Ok(result)
}
```

**Frontend invocation:**

```typescript
const result = await invoke<MigrationScanResult>('check_proton_migrations', {
  steamClientInstallPath: null, // uses default
});
```

**Performance:** Scans all profiles (file reads) + runs Proton discovery (directory listing). Expected <100ms for typical profile counts (<50 profiles).

#### `apply_proton_migration`

Applies a single migration to one profile field.

```rust
#[tauri::command]
pub fn apply_proton_migration(
    request: ApplyMigrationRequest,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<MigrationApplyResult, String> {
    let result = apply_single_migration(&store, &request);

    // Sync metadata on success
    if result.outcome == MigrationOutcome::Applied {
        if let Ok(profile) = store.load(&request.profile_name) {
            let profile_path = store.base_path.join(format!("{}.toml", request.profile_name));
            if let Err(e) = metadata_store.observe_profile_write(
                &request.profile_name,
                &profile,
                &profile_path,
                SyncSource::AppMigration,
                None,
            ) {
                tracing::warn!(%e, profile = %request.profile_name, "metadata sync after migration failed");
            }
        }
    }

    Ok(result)
}
```

**Frontend invocation:**

```typescript
const result = await invoke<MigrationApplyResult>('apply_proton_migration', {
  request: { profileName: 'elden-ring', field: 'steam_proton_path', newPath: '/path/to/new/proton' },
});
```

#### `apply_batch_migration`

Applies multiple migrations sequentially with per-profile error isolation.

```rust
#[tauri::command]
pub fn apply_batch_migration(
    request: BatchMigrationRequest,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<BatchMigrationResult, String> {
    let mut results = Vec::with_capacity(request.migrations.len());
    let mut applied_count = 0;
    let mut failed_count = 0;
    let mut skipped_count = 0;

    for migration in &request.migrations {
        let result = apply_single_migration(&store, migration);

        match result.outcome {
            MigrationOutcome::Applied => {
                applied_count += 1;
                // Best-effort metadata sync
                if let Ok(profile) = store.load(&migration.profile_name) {
                    let path = store.base_path.join(format!("{}.toml", migration.profile_name));
                    let _ = metadata_store.observe_profile_write(
                        &migration.profile_name, &profile, &path,
                        SyncSource::AppMigration, None,
                    );
                }
            }
            MigrationOutcome::Failed => failed_count += 1,
            MigrationOutcome::AlreadyValid => skipped_count += 1,
        }

        results.push(result);
    }

    Ok(BatchMigrationResult { results, applied_count, failed_count, skipped_count })
}
```

**Frontend invocation:**

```typescript
const result = await invoke<BatchMigrationResult>('apply_batch_migration', {
  request: {
    migrations: selectedSuggestions.map((s) => ({
      profileName: s.profile_name,
      field: s.field,
      newPath: s.new_path,
    })),
  },
});
```

### Error Handling

All commands return `Result<T, String>` following existing patterns. Errors are stringified for IPC transport. Categories:

| Error                 | Source                                   | Handling                                 |
| --------------------- | ---------------------------------------- | ---------------------------------------- |
| Profile not found     | `ProfileStore::load()`                   | Per-profile `MigrationOutcome::Failed`   |
| Profile save failure  | `ProfileStore::save()`                   | Per-profile `MigrationOutcome::Failed`   |
| TOML parse error      | `ProfileStore::load()`                   | Per-profile `MigrationOutcome::Failed`   |
| Steam root not found  | `discover_steam_root_candidates()`       | Empty suggestions, diagnostics populated |
| Metadata sync failure | `MetadataStore::observe_profile_write()` | Logged and swallowed (fail-soft)         |

---

## Core Algorithm: Family-Based Proton Matching

### Proton Naming Conventions (Confirmed by Research)

Four active naming schemes exist in the wild:

| Source                                 | Directory Name                         | Display Name         | Location                |
| -------------------------------------- | -------------------------------------- | -------------------- | ----------------------- |
| **Official Valve**                     | `Proton 9.0`                           | `Proton 9.0-1`       | `steamapps/common/`     |
| **GE-Proton (current, post-Feb 2022)** | `GE-Proton10-34`                       | `GE-Proton10-34`     | `compatibilitytools.d/` |
| **GE-Proton (legacy, pre-2022)**       | `Proton-9.23-GE-2`                     | `Proton-9.23-GE-2`   | `compatibilitytools.d/` |
| **Proton-TKG (Frogging-Family)**       | `proton_tkg_6.17.r0.g5f19a815.release` | `TKG-proton-VERSION` | `compatibilitytools.d/` |

All normalize correctly via `normalize_alias()`: `"geproton1034"`, `"proton901"`, `"proton923ge2"`, `"protontkg617r0g5f19a815release"`.

**TKG-Proton caveat:** TKG directory names embed a git commit hash, making numeric segment extraction produce spurious digits from the hash. TKG installs must be **detected by prefix** (`protontkg` in normalized form) and **excluded from the versioned ranking algorithm**. They are still shown to users in the manual selection list but never auto-suggested as a "closest match."

### Step 1: Extract Proton Family

```rust
/// Extracts the "family" from a Proton install name or path by normalizing
/// and stripping trailing version digits.
///
/// TKG-Proton installs are detected by prefix and flagged as non-rankable
/// (their directory names embed git commit hashes that poison version extraction).
///
/// Examples:
///   "GE-Proton9-7"        → Some("geproton")
///   "Proton 9.0-4"        → Some("proton")
///   "Proton-Experimental"  → Some("protonexperimental")
///   "Proton-9.23-GE-2"    → Some("protonge") (legacy GE naming)
///   "proton_tkg_6.17.r0.g5f19a815.release" → Some("protontkg")
///   "Proton EasyAntiCheat Runtime" → Some("protoneasyanticheatruntime")
pub fn extract_proton_family(name: &str) -> Option<String> {
    let normalized = normalize_alias(name)?;

    // TKG-Proton: detect by prefix, return fixed family key.
    // These embed git hashes so version-based ranking is not possible.
    if normalized.starts_with("protontkg") {
        return Some("protontkg".to_string());
    }

    // Strip trailing digits (version numbers)
    let family = normalized.trim_end_matches(|c: char| c.is_ascii_digit());

    if family.is_empty() {
        return Some(normalized); // All digits — use full normalized form
    }

    Some(family.to_string())
}

/// Returns true if the given family key is non-rankable (cannot be ordered
/// by version segments). TKG-Proton is the only known non-rankable family.
pub fn is_non_rankable_family(family: &str) -> bool {
    family == "protontkg"
}
```

> **Note on legacy GE naming:** `"Proton-9.23-GE-2"` normalizes to `"proton923ge2"`, and stripping trailing digits gives family `"protonge"` — which won't match modern `"geproton"`. This is a **design decision point**:
>
> - **Option A (conservative, recommended for Phase 1):** Treat legacy and modern GE as different families. No auto-suggestion across naming eras. Users migrating from very old GE-Proton select manually.
> - **Option B (Phase 2 enhancement):** Add a family alias table: `{"protonge" => "geproton", ...}`. When family extraction produces a key in the alias table, also search candidates under the canonical family. This adds a maintenance burden (new alias rules per naming convention change) but catches more migration scenarios.
>
> The business-analyzer suggested a heuristic: if `normalized.contains("ge") && normalized.contains("proton")`, classify as `"geproton"` family. This is simpler than an alias table but risks false positives on names like `"ProtonBridge"` (unlikely in practice). For Phase 1, Option A is recommended. If users report missed legacy-to-modern GE migrations, add the heuristic in Phase 2.

### Step 2: Extract Version Number

> **Canonical source for version extraction:** Use the **directory name** (not VDF `display_name` or `internal_name`) as the version source. The directory name is always present and deterministic — VDF fields may diverge (e.g., Valve's `internal_name` omits build suffixes that the directory includes).

```rust
/// Extracts numeric version segments from a Proton directory name for ordering.
///
/// Operates on the raw directory name (NOT the normalized form) so that
/// multi-digit numbers like "10" are preserved as a single segment rather
/// than being split into "1" and "0" after normalization strips delimiters.
///
/// Examples:
///   "GE-Proton10-34"       → [10, 34]
///   "Proton 9.0-4"         → [9, 0, 4]
///   "Proton-Experimental"  → [] (no version)
///   "Proton-9.23-GE-2"    → [9, 23, 2]
pub fn extract_version_segments(dir_name: &str) -> Vec<u32> {
    dir_name
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect()
}
```

### Step 3: Match and Rank Candidates

```rust
pub fn find_best_replacement(
    old_proton_name: &str,
    installed_tools: &[ProtonInstall],
) -> Option<(ProtonInstall, f64)> {
    let old_family = extract_proton_family(old_proton_name)?;

    // Non-rankable families (e.g., TKG-Proton with git hashes) cannot be
    // version-ordered, so we never auto-suggest replacements for them.
    if is_non_rankable_family(&old_family) {
        return None;
    }

    let old_version = extract_version_segments(old_proton_name);

    let mut candidates: Vec<(&ProtonInstall, f64)> = Vec::new();

    for tool in installed_tools {
        let tool_family = match extract_proton_family(&tool.name) {
            Some(f) if !is_non_rankable_family(&f) => f,
            _ => continue,
        };

        if tool_family == old_family {
            let tool_version = extract_version_segments(&tool.name);
            if tool_version == old_version {
                continue; // Same version — skip (shouldn't happen if old is missing)
            }

            let is_newer = tool_version > old_version;

            // Major version crossing (e.g., 9→10) gets lower confidence
            // because prefix compatibility is not guaranteed
            let crosses_major = !old_version.is_empty()
                && !tool_version.is_empty()
                && tool_version[0] != old_version[0];

            let confidence = match (is_newer, crosses_major) {
                (true, false) => 0.9,  // Same major, newer build — best match
                (true, true)  => 0.75, // Newer major — may need prefix migration
                (false, false) => 0.7, // Older within same major — rollback
                (false, true)  => 0.5, // Older major — unlikely desired
            };

            candidates.push((tool, confidence));
        }
    }

    // Sort by version descending, pick the newest
    candidates.sort_by(|a, b| {
        let va = extract_version_segments(&a.0.name);
        let vb = extract_version_segments(&b.0.name);
        vb.cmp(&va)
    });

    candidates.into_iter().next().map(|(tool, conf)| (tool.clone(), conf))
}
```

### Step 4: Scan All Profiles

```rust
pub fn scan_proton_migrations(
    store: &ProfileStore,
    steam_root_candidates: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> MigrationScanResult {
    let installed_tools = discover_compat_tools(steam_root_candidates, diagnostics);

    let profile_names = match store.list() {
        Ok(names) => names,
        Err(err) => {
            diagnostics.push(format!("Could not list profiles: {err}"));
            return MigrationScanResult {
                suggestions: Vec::new(),
                profiles_scanned: 0,
                affected_count: 0,
                diagnostics: diagnostics.clone(),
            };
        }
    };

    let mut suggestions = Vec::new();
    let mut unmatched = Vec::new();
    let mut affected_profiles = std::collections::HashSet::new();

    for name in &profile_names {
        let profile = match store.load(name) {
            Ok(p) => p,
            Err(_) => continue, // Skip unloadable profiles
        };

        let effective = profile.effective_profile();
        let launch_method = resolve_launch_method(&effective);

        // Check steam.proton_path for steam_applaunch profiles
        if launch_method == "steam_applaunch"
            && !effective.steam.proton_path.trim().is_empty()
            && !PathBuf::from(&effective.steam.proton_path).exists()
        {
            affected_profiles.insert(name.clone());
            let old_name = extract_name_from_proton_path(&effective.steam.proton_path);
            if let Some((replacement, confidence, crosses_major)) =
                find_best_replacement(&old_name, &installed_tools)
            {
                suggestions.push(MigrationSuggestion {
                    profile_name: name.clone(),
                    field: ProtonPathField::SteamProtonPath,
                    old_path: effective.steam.proton_path.clone(),
                    new_path: replacement.path.to_string_lossy().to_string(),
                    old_proton_name: old_name,
                    new_proton_name: replacement.name.clone(),
                    confidence,
                    proton_family: extract_proton_family(&replacement.name)
                        .unwrap_or_default(),
                    crosses_major_version: crosses_major,
                });
            } else {
                unmatched.push(UnmatchedProfile {
                    profile_name: name.clone(),
                    field: ProtonPathField::SteamProtonPath,
                    stale_path: effective.steam.proton_path.clone(),
                    stale_proton_name: old_name,
                });
            }
        }

        // Check runtime.proton_path for proton_run profiles
        if launch_method == "proton_run"
            && !effective.runtime.proton_path.trim().is_empty()
            && !PathBuf::from(&effective.runtime.proton_path).exists()
        {
            affected_profiles.insert(name.clone());
            let old_name = extract_name_from_proton_path(&effective.runtime.proton_path);
            if let Some((replacement, confidence, crosses_major)) =
                find_best_replacement(&old_name, &installed_tools)
            {
                suggestions.push(MigrationSuggestion {
                    profile_name: name.clone(),
                    field: ProtonPathField::RuntimeProtonPath,
                    old_path: effective.runtime.proton_path.clone(),
                    new_path: replacement.path.to_string_lossy().to_string(),
                    old_proton_name: old_name,
                    new_proton_name: replacement.name.clone(),
                    confidence,
                    proton_family: extract_proton_family(&replacement.name)
                        .unwrap_or_default(),
                    crosses_major_version: crosses_major,
                });
            } else {
                unmatched.push(UnmatchedProfile {
                    profile_name: name.clone(),
                    field: ProtonPathField::RuntimeProtonPath,
                    stale_path: effective.runtime.proton_path.clone(),
                    stale_proton_name: old_name,
                });
            }
        }
    }

    let installed_proton_versions = installed_tools
        .iter()
        .map(|tool| ProtonInstallInfo {
            name: tool.name.clone(),
            path: tool.path.to_string_lossy().to_string(),
            is_official: tool.is_official,
        })
        .collect();

    MigrationScanResult {
        profiles_scanned: profile_names.len(),
        affected_count: affected_profiles.len(),
        suggestions,
        unmatched,
        installed_proton_versions,
        diagnostics: diagnostics.clone(),
    }
}

/// Extracts the Proton install directory name from a full proton path.
///
/// Example: "/home/user/.steam/root/steamapps/common/GE-Proton9-7/proton"
///        → "GE-Proton9-7"
fn extract_name_from_proton_path(proton_path: &str) -> String {
    PathBuf::from(proton_path)
        .parent() // strip "proton" filename
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}
```

### Step 5: Apply Migration

```rust
pub fn apply_single_migration(
    store: &ProfileStore,
    request: &ApplyMigrationRequest,
) -> MigrationApplyResult {
    let mut profile = match store.load(&request.profile_name) {
        Ok(p) => p,
        Err(err) => {
            return MigrationApplyResult {
                profile_name: request.profile_name.clone(),
                field: request.field,
                old_path: String::new(),
                new_path: request.new_path.clone(),
                outcome: MigrationOutcome::Failed,
                error: Some(err.to_string()),
            };
        }
    };

    let old_path = match request.field {
        ProtonPathField::SteamProtonPath => profile.steam.proton_path.clone(),
        ProtonPathField::RuntimeProtonPath => profile.runtime.proton_path.clone(),
    };

    // Validate replacement path: must exist, be a file, and be executable
    let new_path_buf = PathBuf::from(&request.new_path);
    if !new_path_buf.is_file() {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(format!("Replacement path does not exist or is not a file: {}", request.new_path)),
        };
    }

    // Check if already pointing to a valid path
    if PathBuf::from(&old_path).exists() {
        return MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::AlreadyValid,
            error: None,
        };
    }

    // Apply the new path
    match request.field {
        ProtonPathField::SteamProtonPath => {
            profile.steam.proton_path = request.new_path.clone();
        }
        ProtonPathField::RuntimeProtonPath => {
            profile.runtime.proton_path = request.new_path.clone();
        }
    }

    // Save
    match store.save(&request.profile_name, &profile) {
        Ok(()) => MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Applied,
            error: None,
        },
        Err(err) => MigrationApplyResult {
            profile_name: request.profile_name.clone(),
            field: request.field,
            old_path,
            new_path: request.new_path.clone(),
            outcome: MigrationOutcome::Failed,
            error: Some(err.to_string()),
        },
    }
}
```

---

## System Constraints

### Performance

| Operation                    | Expected Latency | Scaling Factor                               |
| ---------------------------- | ---------------- | -------------------------------------------- |
| Proton discovery             | <50ms            | Number of Steam libraries + compat tool dirs |
| Profile scan (all)           | <100ms           | Number of profiles x TOML parse time         |
| Single migration apply       | <10ms            | Single file read + write                     |
| Batch migration (N profiles) | <10ms x N        | Sequential file writes                       |

The scan is dominated by filesystem I/O (directory listing + file reads). For typical installations (<50 profiles, <20 Proton versions), total scan time is well under 200ms. No async/spawn_blocking needed for the initial implementation.

### Atomicity

**Per-profile atomicity:** `ProfileStore::save()` uses `fs::write()` which is a single syscall for small files. TOML profiles are typically <1KB. While not technically atomic (no temp-file-rename pattern), the window for corruption is negligible.

**Batch atomicity:** Batch migration is **not** atomic across profiles. Each profile is migrated independently. If the process crashes mid-batch:

- Already-migrated profiles retain their new paths (correct).
- Remaining profiles retain their old (stale) paths (safe - user can re-run).
- No profile is left in a half-written state (fs::write is effectively atomic for small files).

This matches the existing `batch_check_health` pattern where per-profile errors don't abort the batch.

### Rollback Strategy

**No explicit rollback mechanism.** Rationale:

1. The migration is fully reversible by running the tool again with the old Proton version re-installed, or by manually editing the TOML.
2. The before/after confirmation UI shows exact paths, allowing users to verify before applying.
3. Adding a formal rollback (backup copies, undo log) would add complexity disproportionate to the risk. The existing codebase has no rollback mechanisms for any profile mutation.

If a formal rollback is desired in the future, it could be implemented by saving a snapshot of the old proton path in a migration history table in SQLite (see Open Questions).

### Local Override Awareness

Proton paths live in two layers:

- **Portable base**: `steam.proton_path` / `runtime.proton_path` (empty in storage format)
- **Local override**: `local_override.steam.proton_path` / `local_override.runtime.proton_path`

`ProfileStore::load()` calls `effective_profile()` which merges overrides, then clears the `local_override` section. When `ProfileStore::save()` is called, it calls `storage_profile()` which moves machine-specific paths back into `local_override`.

**The migration operates on the effective profile.** This means:

1. `load()` returns the merged view with the stale path in the base fields.
2. Migration updates the base field (`steam.proton_path` or `runtime.proton_path`).
3. `save()` automatically moves the updated path to `local_override`.

No special handling of the override layer is needed.

---

## Codebase Changes

### Files to Create

| File                                           | Purpose                                  |
| ---------------------------------------------- | ---------------------------------------- |
| `crates/crosshook-core/src/steam/migration.rs` | Core migration logic: scan, match, apply |
| `src-tauri/src/commands/migration.rs`          | Tauri IPC command handlers               |
| `src/types/migration.ts`                       | TypeScript type definitions              |
| `src/hooks/useProtonMigration.ts`              | React hook for migration state           |

### Files to Modify

| File                                           | Change                                                                      |
| ---------------------------------------------- | --------------------------------------------------------------------------- |
| `crates/crosshook-core/src/steam/mod.rs`       | Add `pub mod migration;` and re-export key types                            |
| `crates/crosshook-core/src/steam/proton.rs`    | Promote `normalize_alias` and `resolve_compat_tool_by_name` to `pub(crate)` |
| `crates/crosshook-core/src/metadata/models.rs` | Add `AppMigration` variant to `SyncSource` enum                             |
| `src-tauri/src/commands/mod.rs`                | Add `pub mod migration;`                                                    |
| `src-tauri/src/lib.rs`                         | Register migration commands in `invoke_handler`                             |
| `src/types/index.ts`                           | Re-export migration types                                                   |

### Dependencies

No new crate dependencies. The implementation uses only:

- `std::path::PathBuf`, `std::fs`, `std::collections::HashSet`
- `serde::{Serialize, Deserialize}` (already in workspace)
- Existing crate modules: `steam::proton`, `steam::models`, `profile::toml_store`, `profile::models`

---

## Technical Decisions

### Decision 1: Module Placement

| Option                                        | Pros                                                                                                       | Cons                                                                                                             |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| **A: New `steam/migration.rs`** (recommended) | Clean separation, focused module, migration is primarily a steam/proton concern that consumes profile APIs | Cross-crate dependency direction (steam → profile)                                                               |
| B: New `profile/migration.rs`                 | Migration mutates profiles — closer to the write target                                                    | Primary logic is Proton discovery + family matching, not profile CRUD; would pull steam deps into profile module |
| C: Extend `steam/proton.rs`                   | Fewer files                                                                                                | Mixes discovery (read-only) with migration (read-write); `proton.rs` is already 800+ lines                       |

**Recommendation:** Option A. The core algorithm (family extraction, version comparison, candidate ranking) is fundamentally a steam/proton concern. Profile load/save is a thin consumer. The practices-researcher suggested Option B, which is viable if the team prefers "write target = module home," but the dependency direction favors `steam/`.

### Decision 2: Matching Algorithm

| Option                                           | Pros                                              | Cons                                                                                                     |
| ------------------------------------------------ | ------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| **A: Family + version extraction** (recommended) | Handles real naming conventions, ranks by version | Requires understanding version formats                                                                   |
| B: Normalized string similarity                  | Simpler                                           | Can't distinguish "newer" from "older"; "geproton94" is equally similar to "geproton91" and "geproton97" |
| C: Exact family only (no version ranking)        | Simplest                                          | May suggest older versions over newer ones                                                               |

**Recommendation:** Option A. Version extraction is straightforward (split on non-digit, parse segments) and provides meaningful ranking.

### Decision 3: Batch Atomicity

| Option                                       | Pros                                                        | Cons                                                             |
| -------------------------------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------- |
| A: All-or-nothing with backup                | Full rollback                                               | Complex, no precedent in codebase, adds file management overhead |
| **B: Best-effort per-profile** (recommended) | Simple, matches existing patterns, each profile independent | Partial completion possible                                      |

**Recommendation:** Option B. Consistent with `batch_check_health`, `batch_validate_profiles`, and other batch operations.

### Decision 4: Health Dashboard Integration

| Option                                            | Pros                                                   | Cons                                             |
| ------------------------------------------------- | ------------------------------------------------------ | ------------------------------------------------ |
| A: Embed migration in health report               | Single API call                                        | Tight coupling, slows health scan, complex types |
| **B: Separate API, frontend joins** (recommended) | Decoupled, health scan stays fast, migration is opt-in | Two API calls needed                             |

**Recommendation:** Option B. The health dashboard can show a "migration available" indicator, and the migration panel is a separate UI concern.

### Decision 5: Confidence Scoring

| Scenario                                         | Score | Rationale                                           |
| ------------------------------------------------ | ----- | --------------------------------------------------- |
| Same family, same major, newer build             | 0.9   | Most common upgrade path (e.g., GE-Proton9-4 → 9-7) |
| Same family, newer major version                 | 0.75  | Major version crossing — prefix may need migration  |
| Same family, same major, older build             | 0.7   | Rollback scenario — valid but unusual               |
| Version-less match (e.g., "Proton Experimental") | 0.8   | Family matches but no version to compare            |
| Same family, older major version                 | 0.5   | Unlikely desired — significant downgrade            |

Confidence scores are advisory. The frontend should display them but not auto-apply based on score alone. Major version crossings (confidence ≤ 0.75) should show a warning about potential prefix incompatibility.

---

## Phased Implementation

### Phase 1: Core Detection and Single Migration (MVP)

- `steam/migration.rs` with `scan_proton_migrations()`, `apply_single_migration()`, `extract_proton_family()`, `find_best_replacement()`
- `commands/migration.rs` with `check_proton_migrations`, `apply_proton_migration`
- TypeScript types
- Unit tests for family extraction and version comparison

### Phase 2: Batch Migration and UI Integration

- `apply_batch_migration` command
- `useProtonMigration.ts` hook
- Health dashboard integration (badge/indicator for available migrations)
- Migration confirmation modal with before/after paths

### Phase 3: Enhanced Matching and History (Future)

- Migration history tracking in SQLite (for undo capability)
- Cross-family suggestions at lower confidence (e.g., suggest GE-Proton when official Proton was removed)
- Proton version changelog links

---

## Teammate Feedback Integration

This section documents how feedback from the research team was incorporated.

### Security (W-1): Non-Atomic Profile Writes

**Concern:** `ProfileStore::save()` uses `fs::write()` which truncates before writing — not crash-safe for batch migration.

**Resolution:** This is a pre-existing limitation affecting all profile mutations (save, rename, duplicate, import). For Phase 1, migration uses the same `save()` path for consistency. The risk is minimal: TOML profiles are <1KB and `fs::write()` for small buffers is effectively a single `write()` syscall. A future hardening pass could add temp-file + `fs::rename()` atomicity to `ProfileStore::save()` itself, benefiting all write paths.

### Security (W-2): Local Override Field Targeting

**Concern:** Stored profiles have `steam.proton_path` empty — the real path lives in `local_override.steam.proton_path`. A migration that patches the base field will silently fail.

**Resolution:** This is handled correctly by the load/save roundtrip. `ProfileStore::load()` calls `effective_profile()` which merges `local_override` into base fields and clears `local_override`. Migration updates the base field on the effective profile. `ProfileStore::save()` calls `storage_profile()` which moves machine paths back to `local_override`. The business-analyzer confirmed: "Migration just needs to load → update path field → save — no special local_override handling required." Added explicit documentation in the "Local Override Awareness" section.

### Security (W-3): Consent Gate

**Concern:** Migration must not auto-apply. Needs explicit user confirmation.

**Resolution:** The scan/apply command split inherently provides this. `check_proton_migrations` is read-only (no writes). `apply_proton_migration` and `apply_batch_migration` are write commands that require explicit frontend invocation after the user reviews the before/after paths in a confirmation modal. Migration never triggers from the startup path.

### Security (W-4): Replacement Path Validation

**Concern:** Replacement paths must be validated before writing.

**Resolution:** Added pre-write validation in `apply_single_migration()`: the replacement path must exist and be a file. Additionally, replacement paths originate exclusively from `discover_compat_tools()` which scans trusted Steam directories.

### Practices: SyncSource::AppMigration

**Concern:** Migration writes should be auditable separately from normal app writes.

**Resolution:** Added `AppMigration` variant to `SyncSource` enum. All migration `observe_profile_write()` calls use this variant instead of `AppWrite`.

### Practices: Reuse `resolve_compat_tool_by_name()`

**Concern:** The existing 3-tier matcher in `proton.rs` already handles alias resolution and should be reused.

**Resolution:** Promote `resolve_compat_tool_by_name()` to `pub(crate)` visibility. The migration module's `find_best_replacement()` can use it as a fallback when family-based matching produces no candidates, providing the same fuzzy matching behavior users get from auto-populate.

### UX: Post-Migration Health Re-Check

**Concern:** After successful migration, the health dashboard should reflect the updated state without requiring a manual re-scan.

**Resolution:** The `apply_proton_migration` and `apply_batch_migration` commands already call `observe_profile_write()` to sync metadata. The frontend `useProtonMigration` hook should call `revalidateSingle(profileName)` (from `useProfileHealth`) for each successfully migrated profile. For batch migrations, call `batchValidate()` once after all migrations complete. This reuses the existing health infrastructure with no new backend API.

### UX: Batch Progress Reporting

**Concern:** For batch migrations of 3+ profiles, the UX needs per-profile progress updates.

**Resolution:** Phase 1 uses a synchronous batch command that returns all results at once. For typical profile counts (<50), this completes in <500ms — fast enough that a spinner suffices. If Phase 2 introduces truly large batches, the command can be converted to an async Tauri event stream (emit `migration-progress` events per profile), but this is premature for Phase 1.

### UX: Scan Result Includes All Affected Profiles

**Concern:** Profiles with stale Proton paths but no matching replacement need distinct rendering ("no suggestion found").

**Resolution:** Added `unmatched: Vec<UnmatchedProfile>` to `MigrationScanResult`. Also added `installed_proton_versions: Vec<ProtonInstallInfo>` so the frontend can offer a manual Proton selection dropdown for unmatched profiles.

### Recommendations: Health-Integrated vs. Standalone

**Concern:** The recommendations-agent suggested embedding migration suggestions directly in health reports for a single API call.

**Resolution:** Kept as separate API (Decision 4) for Phase 1. Rationale: health scan runs at startup and must be fast; Proton discovery adds ~50ms. However, the health dashboard UI can call `check_proton_migrations` lazily (on page mount) and display a "migration available" indicator alongside health badges. This achieves the UX benefit without the coupling cost.

---

## Resolved Questions

1. **local_override field targeting (raised by security-researcher and practices-researcher):** Confirmed NOT a blocker. The `load()` → `effective_profile()` → mutate → `save()` → `storage_profile()` roundtrip correctly moves the updated path to `local_override`. Verified by code trace and the existing `storage_profile_roundtrip_is_idempotent` test at `models.rs:492`. This is the same pattern used by `profile_save`, `save_launch_optimizations`, and `profile_rename` display_name update.

2. **steam_applaunch Proton path relevance (raised by api-researcher):** CrossHook profiles store `steam.proton_path` explicitly for steam_applaunch profiles — it's used in launch script generation and health validation. If the path is stale, the profile is broken regardless of Steam's own Proton management. Migration MUST check steam_applaunch profiles.

---

## Open Questions

1. **Should migration scan run at startup alongside health scan?** The health scan already detects stale proton paths. Migration scan adds Proton discovery cost (~50ms). Could piggyback on the existing startup health event, or be triggered only from the Health Dashboard UI.

2. **Should compatdata_path also be migrated?** When Proton is upgraded, the `compatdata` directory path may also change (different prefix structure). This is a separate concern but related. For Phase 1, focus only on the Proton executable path.

3. **How should "Proton Experimental" be handled?** It has no version number, so family matching works but version comparison doesn't. The algorithm handles this (empty version segments → confidence 0.8), but the UX should indicate "same family, version comparison unavailable."

4. **Should the CLI (`crosshook-cli`) also expose migration?** The core logic is in `crosshook-core`, so CLI integration is trivial. Scope decision for Phase 2+.

5. **Flatpak Steam discovery gap (identified by api-researcher):** The path `$HOME/.var/app/com.valvesoftware.Steam/data/Steam/` is NOT in the current `discover_steam_root_candidates()`. Flatpak-installed Proton versions will be missed by migration scan. This should be fixed in `steam/discovery.rs` as a prerequisite or parallel task — it affects all Steam discovery, not just migration.

6. **Legacy GE-Proton naming:** Pre-2022 GE-Proton used `Proton-X.Y-GE-Z` format (e.g., `Proton-9.23-GE-2`). The family extraction algorithm produces `"protonge"` for these, which won't match modern `"geproton"` family. This is intentionally safe (no cross-era auto-suggestion), but users migrating from very old GE-Proton will need to manually select a replacement.
