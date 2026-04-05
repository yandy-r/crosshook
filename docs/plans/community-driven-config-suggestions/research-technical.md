# ML-Assisted Configuration — Technical Specification

## Executive Summary

CrossHook already has a mature ProtonDB integration (`crosshook-core/src/protondb/`) that fetches game compatibility data, extracts environment variable suggestions from community report text, and caches results in SQLite. The ML-assisted configuration feature builds on this foundation with a **three-tier suggestion architecture**: (1) direct mapping of ProtonDB env vars to existing optimization catalog entries for one-click toggle activation, (2) frequency-weighted raw env var suggestions from report aggregation, and (3) future ML model trained on ProtonDB corpus. No new external APIs or dependencies are required for Phase 1. The existing aggregation pipeline, cache layer, optimization catalog, and IPC patterns are reused directly.

---

## Architecture Design

### Component Diagram

```
                          +-----------------------+
                          |    ProtonDB Servers    |
                          |   (public, no auth)    |
                          +-----------+-----------+
                                      |
                          [reqwest, TLS, 6s timeout]
                                      |
                          +-----------v-----------+
                          |  protondb/client.rs   |  <-- existing
                          |  (summary + reports)  |
                          +-----------+-----------+
                                      |
                          +-----------v-----------+
                          | protondb/aggregation  |  <-- existing
                          | .rs                   |
                          | (env var extraction,  |
                          |  launch option parse) |
                          +-----------+-----------+
                                      |
                   +------------------v------------------+
                   |        protondb/suggestions.rs      |  <-- NEW
                   | Tier 1: catalog bridge (env->toggle)|
                   | Tier 2: heuristic raw env vars      |
                   | Compare against GameProfile state   |
                   +------------------+------------------+
                          |                       |
            +-------------v-----------+   +-------v-----------+
            | launch/catalog.rs       |   | commands/protondb |
            | OptimizationEntry.env   |   | .rs (Tauri IPC)   |
            | (21 known env mappings) |   +-------+-----------+
            +-------------------------+           |
                                      [Serde JSON over IPC]
            +-------------------------+           |
            | MetadataStore           |   +-------v-----------+
            | external_cache_entries  |   | useProtonDb       |
            | (existing cache layer)  |   | Suggestions.ts    |
            +-------------------------+   +-------------------+
```

### New Modules

| Module           | Location                                      | Purpose                                                                               |
| ---------------- | --------------------------------------------- | ------------------------------------------------------------------------------------- |
| `suggestions.rs` | `crosshook-core/src/protondb/suggestions.rs`  | Three-tier suggestion engine: catalog bridge + heuristic scoring + profile comparison |
| Tauri commands   | `src-tauri/src/commands/protondb.rs` (extend) | New IPC endpoints for fetching, accepting, and dismissing suggestions                 |
| Frontend hook    | `src/hooks/useProtonDbSuggestions.ts`         | React hook wrapping suggestion IPC with loading states                                |
| Frontend types   | `src/types/protondb.ts` (extend)              | TypeScript interfaces for suggestion data                                             |

### Integration Points

The suggestion engine takes three inputs:

1. **`ProtonDbLookupResult`** — from the existing `lookup_protondb()` function (which handles caching, stale fallback, and offline mode)
2. **`GameProfile`** — the current profile state loaded from TOML
3. **`&[OptimizationEntry]`** — the loaded optimization catalog (from `global_catalog()`)

It produces a **`ProtonDbSuggestionSet`** that groups suggestions into two tiers: catalog-mapped toggles (Tier 1) and raw env var suggestions (Tier 2), each with conflict/duplicate detection against the profile's existing state.

---

## Data Models

### Rust Structs (crosshook-core)

```rust
// --- protondb/suggestions.rs ---

use serde::{Deserialize, Serialize};
use super::models::{
    ProtonDbEnvVarSuggestion, ProtonDbLaunchOptionSuggestion,
    ProtonDbAdvisoryNote, ProtonDbCacheState,
};

/// Status of a single suggestion relative to the current profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionStatus {
    /// Not yet applied; user can accept.
    New,
    /// Already present in profile with the same value.
    AlreadyApplied,
    /// Present in profile but with a different value.
    Conflict,
    /// User dismissed this suggestion (runtime-only state in Phase 1).
    Dismissed,
}

/// Tier 1: A suggestion that maps directly to an optimization catalog entry.
/// Accepting this enables the catalog toggle (via enabled_option_ids) instead
/// of writing a raw env var.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogSuggestionItem {
    /// The catalog entry ID (e.g. "enable_dxvk_async").
    pub catalog_entry_id: String,
    /// Human-readable label from the catalog (e.g. "Enable DXVK async").
    pub label: String,
    /// Catalog description.
    pub description: String,
    /// The underlying env var(s) this toggle controls.
    pub env_vars: Vec<ProtonDbEnvVarSuggestion>,
    /// How many ProtonDB reports recommended this.
    pub supporting_report_count: u32,
    /// Current status relative to the profile.
    pub status: SuggestionStatus,
    /// Category from catalog (e.g. "graphics", "compatibility").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub category: String,
    /// GPU vendor filter from catalog (e.g. "nvidia", "amd", or empty for all).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub target_gpu_vendor: String,
}

/// Tier 2: A raw env-var suggestion with no catalog mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarSuggestionItem {
    /// The underlying ProtonDB suggestion data (key, value, source_label, count).
    pub suggestion: ProtonDbEnvVarSuggestion,
    /// Current status relative to the profile.
    pub status: SuggestionStatus,
    /// If status is Conflict, the current value in the profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_value: Option<String>,
}

/// A single launch-option suggestion with status context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchOptionSuggestionItem {
    pub suggestion: ProtonDbLaunchOptionSuggestion,
    pub status: SuggestionStatus,
}

/// Aggregated suggestion set for a single game/profile combination.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProtonDbSuggestionSet {
    /// Steam App ID this suggestion set belongs to.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub app_id: String,
    /// Profile name this suggestion set was computed against.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub profile_name: String,
    /// Tier 1: Suggestions that map to optimization catalog entries (highest confidence).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalog_suggestions: Vec<CatalogSuggestionItem>,
    /// Tier 2: Raw env var suggestions with no catalog mapping.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_var_suggestions: Vec<EnvVarSuggestionItem>,
    /// Copy-only launch option strings (not auto-applicable).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub launch_option_suggestions: Vec<LaunchOptionSuggestionItem>,
    /// Advisory notes from community reports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub advisory_notes: Vec<ProtonDbAdvisoryNote>,
    /// Cache freshness metadata (from the underlying ProtonDB lookup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<ProtonDbCacheState>,
    /// Total number of new (actionable) suggestions across all tiers.
    pub actionable_count: u32,
    /// Timestamp when this suggestion set was computed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub computed_at: String,
}

/// Request to accept a suggestion into a profile.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind")]
pub enum AcceptSuggestionRequest {
    /// Accept a Tier 1 catalog suggestion (enables the optimization toggle).
    #[serde(rename = "catalog")]
    Catalog {
        profile_name: String,
        catalog_entry_id: String,
    },
    /// Accept a Tier 2 raw env var suggestion.
    #[serde(rename = "env_var")]
    EnvVar {
        profile_name: String,
        env_key: String,
        env_value: String,
    },
}

/// Result of accepting a suggestion.
#[derive(Debug, Clone, Serialize)]
pub struct AcceptSuggestionResult {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
```

### SQLite Schema

**No new tables required for Phase 1.** The feature reuses:

| Table                    | Usage                                                                  |
| ------------------------ | ---------------------------------------------------------------------- |
| `external_cache_entries` | Cache ProtonDB API responses (existing, namespace `protondb:{app_id}`) |

**Phase 2 (optional, if dismissed suggestion persistence is needed):**

```sql
-- Schema v17 (deferred)
CREATE TABLE IF NOT EXISTS suggestion_feedback (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id       TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    app_id           TEXT NOT NULL,
    suggestion_key   TEXT NOT NULL,  -- e.g. "catalog:enable_dxvk_async" or "env:PROTON_USE_WINED3D=1"
    action           TEXT NOT NULL,  -- 'accepted' | 'dismissed'
    created_at       TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_suggestion_feedback_profile_app
    ON suggestion_feedback(profile_id, app_id);
```

### Persistence Classification

| Datum                      | Layer                                     | Notes                                                                         |
| -------------------------- | ----------------------------------------- | ----------------------------------------------------------------------------- |
| ProtonDB API responses     | SQLite `external_cache_entries`           | Existing, 6h TTL, 512 KiB cap                                                 |
| Derived suggestion set     | Runtime-only (memory)                     | Computed on-demand from cached lookup + profile state + catalog               |
| Accepted catalog toggles   | TOML profile                              | Written to `[launch.optimizations.enabled_option_ids]` via existing save path |
| Accepted raw env vars      | TOML profile                              | Written to `[launch.custom_env_vars]` via existing save path                  |
| Dismissed suggestion state | Runtime-only (Phase 1) / SQLite (Phase 2) | Loses state on restart in Phase 1                                             |

---

## API Design

### Tauri IPC Commands

#### `protondb_get_suggestions`

Fetches or derives suggestions for a game profile. Internally calls `lookup_protondb()` (which handles caching), then runs the three-tier suggestion engine against the current profile state and optimization catalog.

```rust
#[tauri::command]
pub async fn protondb_get_suggestions(
    app_id: String,
    profile_name: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
    profile_store: State<'_, ProfileStore>,
) -> Result<ProtonDbSuggestionSet, String>
```

**Request parameters:**

- `app_id: String` — Steam App ID (e.g. `"1245620"`)
- `profile_name: String` — Profile filename to compare against
- `force_refresh: Option<bool>` — Bypass cache and re-fetch from ProtonDB

**Response:** `ProtonDbSuggestionSet` (see struct above)

**Errors:** Returns `String` error message (consistent with existing command pattern)

**Frontend invocation:**

```typescript
const result = await invoke<ProtonDbSuggestionSet>('protondb_get_suggestions', {
  appId: '1245620',
  profileName: 'my-game.toml',
  forceRefresh: false,
});
```

#### `protondb_accept_suggestion`

Accepts a suggestion by writing it to the profile. For Tier 1 catalog suggestions, enables the optimization toggle ID in `enabled_option_ids`. For Tier 2 raw env vars, writes to `custom_env_vars`.

```rust
#[tauri::command]
pub async fn protondb_accept_suggestion(
    request: AcceptSuggestionRequest,
    profile_store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<AcceptSuggestionResult, String>
```

**Request:** `AcceptSuggestionRequest` (tagged enum: `Catalog { profile_name, catalog_entry_id }` or `EnvVar { profile_name, env_key, env_value }`)

**Response:** `AcceptSuggestionResult` with success flag

**Validation (re-validated at write time, not trusted from cache):**

- **Catalog path**: `catalog_entry_id` must exist in the loaded catalog
- **Env var path**: `env_key` must pass `is_safe_env_key()`, `env_value` must pass `is_safe_env_value()`, key must not be in `RESERVED_ENV_KEYS`
- Both paths: profile must exist and be loadable

**Write path (Catalog):**

1. Load profile via `ProfileStore::load()`
2. Add `catalog_entry_id` to `launch.optimizations.enabled_option_ids` if not present
3. Save via `ProfileStore::save()`
4. Record config revision with source `ProtonDbSuggestion`

**Write path (EnvVar):**

1. Load profile via `ProfileStore::load()`
2. Insert/update `launch.custom_env_vars[env_key] = env_value`
3. Save via `ProfileStore::save()`
4. Record config revision with source `ProtonDbSuggestion`

#### `protondb_dismiss_suggestion`

Dismisses a suggestion for the current session (runtime-only in Phase 1).

```rust
#[tauri::command]
pub async fn protondb_dismiss_suggestion(
    profile_name: String,
    suggestion_key: String,
) -> Result<(), String>
```

**Note:** In Phase 1, this is a no-op on the backend (dismissal tracked in frontend state). In Phase 2, this writes to `suggestion_feedback` table.

---

## Text Extraction Pipeline

### Existing Infrastructure (No New Work Needed)

The existing `protondb/aggregation.rs` already implements a robust text extraction pipeline:

1. **Input**: ProtonDB report feed JSON (`ProtonDbReportFeedResponse`) containing an array of user reports, each with `launch_options` (freeform text) and `concluding_notes`

2. **Env Var Extraction** (`safe_env_var_suggestions()`):
   - Splits `launch_options` text at `%command%` to isolate the env prefix
   - Tokenizes on whitespace
   - Extracts `KEY=VALUE` pairs where key passes `is_safe_env_key()` and value passes `is_safe_env_value()`
   - Filters out reserved keys (`WINEPREFIX`, `STEAM_COMPAT_*`)
   - Groups identical env var sets across reports, counts occurrences

3. **Launch Option Extraction**:
   - Raw launch strings that contain non-env-var tokens (executables, flags, quoted values) are classified as "copy-only"
   - These are presented for manual copying, not auto-application
   - Ranked by frequency across reports

4. **Aggregation** (`normalize_report_feed()`):
   - Groups env var suggestions by signature (sorted key=value pairs)
   - Ranks groups by report count (descending)
   - Caps output: 3 env groups, 3 launch groups, 4 note groups, 3 notes per group
   - Produces `Vec<ProtonDbRecommendationGroup>` which is already part of `ProtonDbSnapshot`

### Three-Tier Suggestion Architecture

**Tier 1 -- Catalog Bridge (direct mapping, highest confidence):**

ProtonDB env var suggestions are matched against the existing `optimization_catalog` (22 entries in `assets/default_optimization_catalog.toml`). When a ProtonDB suggestion like `DXVK_ASYNC=1` maps to catalog entry `enable_dxvk_async`, the suggestion is surfaced as "Enable this optimization toggle" rather than a raw env var -- giving users the full catalog UX (label, description, help text, conflict detection, GPU vendor filtering).

Known env-to-catalog mappings (built at runtime from loaded catalog `env` fields):

| ProtonDB Env Var                     | Catalog Entry ID            | Label                             |
| ------------------------------------ | --------------------------- | --------------------------------- |
| `PROTON_NO_STEAMINPUT=1`             | `disable_steam_input`       | Disable Steam Input               |
| `PROTON_PREFER_SDL=1`                | `prefer_sdl_input`          | Prefer SDL controller handling    |
| `PROTON_NO_WM_DECORATION=1`          | `hide_window_decorations`   | Hide window decorations           |
| `PROTON_ENABLE_HDR=1`                | `enable_hdr`                | Enable HDR                        |
| `PROTON_ENABLE_WAYLAND=1`            | `enable_wayland_driver`     | Use native Wayland support        |
| `PROTON_USE_NTSYNC=1`                | `use_ntsync`                | Use NTSync                        |
| `PROTON_NO_ESYNC=1`                  | `disable_esync`             | Disable esync                     |
| `PROTON_NO_FSYNC=1`                  | `disable_fsync`             | Disable fsync                     |
| `PROTON_ENABLE_NVAPI=1`              | `enable_nvapi`              | Enable NVAPI support              |
| `PROTON_FORCE_LARGE_ADDRESS_AWARE=1` | `force_large_address_aware` | Force Large Address Aware         |
| `PROTON_LOG=1`                       | `enable_proton_log`         | Enable Proton logging             |
| `PROTON_LOCAL_SHADER_CACHE=1`        | `enable_local_shader_cache` | Isolate shader cache per game     |
| `DXVK_ASYNC=1`                       | `enable_dxvk_async`         | Enable DXVK async                 |
| `DXVK_FRAME_RATE=60`                 | `cap_dxvk_frame_rate`       | Cap DXVK frame rate (60 FPS)      |
| `VKD3D_CONFIG=dxr`                   | `enable_vkd3d_dxr`          | Enable VKD3D DXR                  |
| `PROTON_FSR4_UPGRADE=1`              | `enable_fsr4_upgrade`       | Auto-upgrade FSR4                 |
| `PROTON_XESS_UPGRADE=1`              | `enable_xess_upgrade`       | Auto-upgrade XeSS                 |
| `PROTON_DLSS_UPGRADE=1`              | `enable_dlss_upgrade`       | Auto-upgrade DLSS                 |
| `PROTON_DLSS_INDICATOR=1`            | `show_dlss_indicator`       | Show DLSS indicator               |
| `PROTON_NVIDIA_LIBS=1`               | `enable_nvidia_libs`        | Enable NVIDIA game libraries      |
| `SteamDeck=1`                        | `steamdeck_compat_mode`     | Use Steam Deck compatibility mode |

The mapping is built at runtime from the loaded catalog's `env` field, so user-added catalog entries are automatically included:

```rust
/// Build a lookup from (env_key, env_value) -> catalog_entry_id.
fn build_catalog_env_index(catalog: &[OptimizationEntry]) -> HashMap<(String, String), String> {
    let mut index = HashMap::new();
    for entry in catalog {
        for pair in &entry.env {
            index.insert((pair[0].clone(), pair[1].clone()), entry.id.clone());
        }
    }
    index
}
```

**Tier 2 -- Heuristic Scoring (frequency-weighted, existing aggregation):**

ProtonDB suggestions that do not map to a catalog entry are surfaced as raw env var suggestions with `supporting_report_count` for confidence ranking. This is what the existing aggregation pipeline already produces. These go to `custom_env_vars` in the profile when accepted.

**Tier 3 -- ML Model (future, deferred):**

A model trained on the ProtonDB corpus to predict optimal configurations based on game characteristics, hardware profile, and Proton version. Only justified after Tier 1 and Tier 2 prove insufficient in user testing.

### Suggestion Engine (New Work)

The new `suggestions.rs` module adds a **comparison and classification layer** on top of the existing aggregation:

```rust
pub fn derive_suggestions(
    lookup: &ProtonDbLookupResult,
    profile: &GameProfile,
    catalog: &[OptimizationEntry],
    dismissed_keys: &HashSet<String>,
) -> ProtonDbSuggestionSet {
    let catalog_env_index = build_catalog_env_index(catalog);

    // 1. Extract all env var suggestions from recommendation_groups
    // 2. For each suggestion, check if (key, value) maps to a catalog entry:
    //    - If mapped: produce CatalogSuggestionItem (Tier 1) with catalog metadata
    //      Check status against profile.launch.optimizations.enabled_option_ids
    //    - If not mapped: produce EnvVarSuggestionItem (Tier 2)
    //      Check status against profile.launch.custom_env_vars
    // 3. For each item, determine status:
    //    - Not present in profile -> SuggestionStatus::New
    //    - Present with same value -> SuggestionStatus::AlreadyApplied
    //    - Present with different value -> SuggestionStatus::Conflict
    //    - In dismissed_keys set -> SuggestionStatus::Dismissed
    // 4. Sort each tier: New first (by report count desc), then Conflict, then AlreadyApplied
    // 5. Produce advisory_notes from community notes groups
    // 6. Count actionable (New) suggestions across both tiers
}
```

### Regex Patterns (Already Implemented)

The env var key validation uses character-class matching, not regex:

- **Key**: First char must be `[A-Z_]`, remaining chars `[A-Z0-9_]`
- **Value**: Rejects any of: `$ ; " ' \` | & < > ( ) %` and whitespace and null bytes

These patterns are in `aggregation.rs` lines 300-322 and are battle-tested with existing unit tests.

---

## System Constraints

### Performance

- **Catalog index build**: O(c) where c = catalog entries (currently 22). Built once per suggestion request.
- **Suggestion derivation**: O(n\*m) where n = env var suggestions (max ~9 from 3 groups) and m = profile custom_env_vars (typically <20). Near-instant.
- **ProtonDB API latency**: 6-second timeout, typically <2s. Cached for 6 hours.
- **Cache size**: ProtonDB lookup payloads are typically 10-50 KiB, well within the 512 KiB cap.
- **No background processing**: Suggestions are derived on-demand when the user views a profile, not pre-computed.

### Caching Strategy

1. User opens profile details or suggestion panel
2. Frontend calls `protondb_get_suggestions(app_id, profile_name)`
3. Backend calls `lookup_protondb()` which checks `external_cache_entries` (6h TTL)
4. If cache hit: deserialize `ProtonDbLookupResult`, load catalog, derive suggestions immediately
5. If cache miss: fetch from ProtonDB, cache result, then derive suggestions
6. If offline + stale cache: derive suggestions from stale data, mark `cache.is_stale = true`
7. If offline + no cache: return empty suggestion set with `state: unavailable`

### Scalability

- Per-game cache entries: one entry per Steam App ID in `external_cache_entries`
- No cross-game aggregation needed (suggestions are per-game)
- No background sync (on-demand only)
- Memory: suggestion sets are ephemeral, GC'd when component unmounts

---

## Codebase Changes

### Files to Create

| File                                                | Description                                                                             |
| --------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/protondb/suggestions.rs` | Three-tier suggestion engine: `derive_suggestions()`, catalog bridge, status comparison |
| `src/hooks/useProtonDbSuggestions.ts`               | Frontend hook: loading states, IPC wrapper, dismissed set management                    |

### Files to Modify

| File                                                | Change                                                                                                                           |
| --------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/protondb/mod.rs`         | Add `pub mod suggestions;` and re-export suggestion types                                                                        |
| `crates/crosshook-core/src/protondb/aggregation.rs` | Make `is_safe_env_key()` and `is_safe_env_value()` `pub(crate)` (currently private, needed for accept-time re-validation)        |
| `src-tauri/src/commands/protondb.rs`                | Add `protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion` commands                             |
| `src-tauri/src/lib.rs`                              | Register 3 new commands in `invoke_handler`                                                                                      |
| `src/types/protondb.ts`                             | Add TypeScript interfaces for `ProtonDbSuggestionSet`, `CatalogSuggestionItem`, `EnvVarSuggestionItem`, `SuggestionStatus`, etc. |

### Dependencies

**No new crate dependencies.** The feature uses only existing dependencies:

- `serde` / `serde_json` -- serialization
- `chrono` -- timestamps
- `reqwest` -- HTTP (existing ProtonDB client)
- `rusqlite` -- cache (existing)

---

## Technical Decisions

### Decision 1: Three-tier suggestion architecture

**Chosen: Tiered approach (Catalog Bridge > Heuristic Scoring > ML)**

| Approach                       | Confidence                                    | Effort                     | User Value                                                               |
| ------------------------------ | --------------------------------------------- | -------------------------- | ------------------------------------------------------------------------ |
| Tier 1: Catalog Bridge         | Highest -- known, tested optimization entries | Low -- index lookup        | One-click toggle with full UX (labels, descriptions, conflict detection) |
| Tier 2: Heuristic raw env vars | Medium -- frequency-weighted from community   | Low -- already implemented | Copy/apply with report count confidence indicator                        |
| Tier 3: ML model               | Varies -- requires training data              | High -- new infrastructure | Predictive configuration based on game/hardware characteristics          |

**Rationale:** Most ProtonDB env var suggestions (21 of the common ones) map directly to existing optimization catalog entries. Tier 1 delivers the highest-quality UX with the least implementation effort. Tier 2 catches the long tail of unusual env vars. Tier 3 is only justified after Tiers 1-2 are deployed and user feedback indicates a gap.

### Decision 2: Extend protondb/ module vs. new top-level module

**Chosen: Extend `protondb/`**

**Rationale:** The suggestion engine is semantically part of ProtonDB data processing. It consumes `ProtonDbLookupResult` and produces suggestions. Keeping it in `protondb/` maintains cohesion and follows the existing module organization pattern (e.g., `protondb/aggregation.rs` is already separate from `protondb/client.rs`).

### Decision 3: On-demand derivation vs. separate cached suggestion layer

**Chosen: On-demand derivation (no separate suggestion cache)**

**Rationale:** Suggestion derivation is O(1) in practice (< 10 suggestions, < 20 profile env vars, 22 catalog entries). Caching the derivation would introduce staleness bugs when the user modifies their profile -- the cached suggestions wouldn't reflect the change. Computing on-demand ensures correctness.

### Decision 4: Suggestion feedback persistence

**Chosen: Phase 1 runtime-only, Phase 2 SQLite table**

**Rationale:** ProtonDB data refreshes every 6 hours. Dismissed suggestions may become irrelevant after a refresh (e.g., new reports change the top suggestions). Runtime-only dismissal is sufficient for MVP. If users report frustration with re-appearing suggestions, Phase 2 adds the `suggestion_feedback` table with schema migration v17.

### Decision 5: Accept via dedicated command vs. generic profile_save

**Chosen: Dedicated `protondb_accept_suggestion` command**

**Rationale:** The dedicated command provides:

1. **Tier-aware routing**: Catalog suggestions write to `enabled_option_ids`; raw env vars write to `custom_env_vars`
2. **Re-validation**: Env key/value safety checked at write time (not trusted from cache)
3. **Audit trail**: Config revision recorded with source `ProtonDbSuggestion`
4. **Atomicity**: Single IPC call loads, merges, saves, and records revision

### Decision 6: ML approach (future Phase 3)

**Deferred.** Phase 1 uses catalog bridge mapping. Phase 2 could add:

- Weighted scoring factoring in report recency, Proton version match, and hardware similarity
- Clustering of env var combinations that commonly appear together
- Collaborative filtering: "users who launched this game also used these env vars"

This requires a larger ProtonDB data corpus than the current single-page report feed provides.

---

## Open Questions

1. **Bulk accept**: Should the UI allow accepting all new suggestions at once, or only one at a time? Bulk accept is simpler for users but may write conflicting env vars. Recommendation: one-at-a-time with visual grouping to show related suggestions. For Tier 1 catalog suggestions, bulk-accept is safer since the catalog already handles conflicts.

2. **Proton version filtering**: The report feed includes `proton_version` per report. Should suggestions be filtered to only show env vars from reports matching the user's current Proton version? This would reduce noise but also reduce suggestion count. Recommendation: show all suggestions but display the Proton version badge for context.

3. **PCGamingWiki as alternative data source**: PCGamingWiki has structured data (not freeform text) that could feed the same suggestion pipeline. Requires HTML scraping or a Cargo API client. Recommendation: defer to Phase 2 as a parallel data source once the ProtonDB pipeline is proven.

4. **Rate limiting**: The existing ProtonDB client has no rate limiting beyond the 6-second timeout. Should we add request debouncing if users rapidly navigate between games? The frontend hook should handle this with the existing requestIdRef pattern from `useProtonDbLookup.ts`.

---

## Relevant Files (Deep References)

### Existing ProtonDB module (reuse as-is)

- `crates/crosshook-core/src/protondb/mod.rs` -- Module root, re-exports
- `crates/crosshook-core/src/protondb/client.rs` -- HTTP client, cache read/write, stale fallback
- `crates/crosshook-core/src/protondb/aggregation.rs` -- Report text parsing, env var extraction, safety validation
- `crates/crosshook-core/src/protondb/models.rs` -- All ProtonDB data types
- `crates/crosshook-core/src/protondb/tests.rs` -- Existing test patterns

### Optimization catalog (Tier 1 bridge source)

- `assets/default_optimization_catalog.toml` -- 22 entries with `env` field mappings (the source for Tier 1 catalog bridge)
- `crates/crosshook-core/src/launch/catalog.rs` -- `OptimizationEntry` struct, `global_catalog()`, `parse_catalog_toml()`

### Cache infrastructure

- `crates/crosshook-core/src/metadata/cache_store.rs` -- `get_cache_entry`, `put_cache_entry`, `evict_expired_cache_entries`
- `crates/crosshook-core/src/metadata/mod.rs` -- MetadataStore public API
- `crates/crosshook-core/src/metadata/db.rs` -- `new_id()`, connection configuration

### Profile models

- `crates/crosshook-core/src/profile/models.rs` -- `GameProfile`, `LaunchSection.custom_env_vars`, `LaunchOptimizationsSection.enabled_option_ids`
- `crates/crosshook-core/src/profile/toml_store.rs` -- `ProfileStore::save()`

### Tauri command patterns

- `src-tauri/src/commands/protondb.rs` -- Existing `protondb_lookup` command (extend this file)
- `src-tauri/src/commands/profile.rs` -- Profile save command patterns (includes `profile_save_launch_optimizations`)
- `src-tauri/src/lib.rs` -- Command registration (lines 209-314)

### Frontend patterns

- `src/hooks/useProtonDbLookup.ts` -- Hook pattern to follow (loading states, request cancellation, stale-while-revalidate)
- `src/hooks/useLaunchOptimizationCatalog.ts` -- Catalog hook pattern for Tier 1 UI
- `src/types/protondb.ts` -- TypeScript type definitions to extend

### Schema and migrations

- `crates/crosshook-core/src/metadata/migrations.rs` -- Migration pattern (current schema v16)
- `crates/crosshook-core/src/metadata/models.rs` -- Row types, error types, constants

All paths are relative to `src/crosshook-native/`.
