# Integration Research: Community-Driven Config Suggestions

The ProtonDB integration is mature and already performs env-var extraction at the aggregation layer. The profile data model has direct fields for env vars and optimization IDs. The missing pieces are: a backend command to apply suggestions atomically, and a frontend flow to surface the suggestions during profile creation/editing.

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — async `lookup_protondb()` entry point, caching logic, stale fallback
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs` — env-var extraction, safety filtering, `normalize_report_feed()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs` — `ProtonDbLookupResult`, `ProtonDbSnapshot`, `ProtonDbRecommendationGroup`, `ProtonDbEnvVarSuggestion`, `ProtonDbTier`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs` — Tauri IPC: `protondb_lookup` command
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs` — all profile IPC commands, `capture_config_revision()`, `ConfigRevisionSource`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `GameProfile`, `LaunchSection`, `LaunchOptimizationsSection`, `custom_env_vars: BTreeMap<String, String>`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs` — `OptimizationCatalog`, `OptimizationEntry`, `global_catalog()`, `load_catalog()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/env.rs` — `RESERVED_ENV_KEYS`, `WINE_ENV_VARS_TO_CLEAR`, `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `get_cache_entry()`, `put_cache_entry()`, `evict_expired_cache_entries()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` public API: `put_cache_entry`, `get_cache_entry`, `insert_config_revision`, `lookup_profile_id`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — full schema through v16
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/protondb.ts` — TypeScript type mirror of all Rust ProtonDB models
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts` — React hook: `useProtonDbLookup(appId)`, request deduplication, refresh
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/utils/protondb.ts` — `mergeProtonDbEnvVarGroup()`, conflict detection types, `PendingProtonDbOverwrite`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/utils/optimization-catalog.ts` — `fetchOptimizationCatalog()`, `buildOptionsById()`, `buildConflictMatrix()`

## Architectural Patterns

- **Single IPC command for ProtonDB**: `protondb_lookup(app_id, force_refresh?)` → `ProtonDbLookupResult`. No pagination, no filtering — full snapshot returned to frontend.
- **Aggregation-time safety**: `safe_env_var_suggestions()` in `aggregation.rs` filters env vars before they ever reach `external_cache_entries`. The frontend receives only safe, pre-validated key-value pairs.
- **Cache upsert on conflict**: `external_cache_entries` uses `ON CONFLICT(cache_key) DO UPDATE SET ...`. The cache key is `"protondb:{app_id}"`. Re-fetch replaces the existing row.
- **Stale-while-revalidate with explicit state**: `ProtonDbLookupState` enum (`idle`/`loading`/`ready`/`stale`/`unavailable`) drives UI. On network failure, the client returns stale cache with `is_stale=true` rather than an error.
- **Profile write chain**: every profile mutation must call `observe_profile_write()` then `capture_config_revision()`. Config revisions are content-hash-deduped — identical saves produce no new row.
- **Optimization catalog as singleton**: `global_catalog()` returns a process-wide `OnceLock<OptimizationCatalog>`. The allowed env key set (`allowed_env_keys: HashSet<String>`) is derived at initialization from all catalog `env` entries.
- **Frontend conflict resolution**: `mergeProtonDbEnvVarGroup()` computes `conflicts`, `appliedKeys`, `unchangedKeys`. The `PendingProtonDbOverwrite` type carries pending resolution state for UI confirmation flows.

## ProtonDB API Client

**Endpoints consumed:**

- `GET https://www.protondb.com/api/v1/reports/summaries/{app_id}.json` — tier, score, confidence, total reports
- `GET https://www.protondb.com/data/counts.json` — report count + timestamp (used for report feed URL hash)
- `GET https://www.protondb.com/data/reports/all-devices/app/{hash}.json` — paginated freeform report feed

**URL hashing algorithm** (`aggregation.rs:report_feed_id()`):
The report feed URL contains an integer hash derived from `app_id × counts_data`. The client fetches `counts.json` first, computes the hash, then fetches the feed. On 404, it refetches `counts.json` and retries once. On second 404, returns `HashResolutionFailed`.

**Request configuration:**

- Timeout: 6 seconds (`REQUEST_TIMEOUT_SECS = 6`)
- User-Agent: `CrossHook/{crate_version}`
- HTTP client: `reqwest::Client` initialized as `OnceLock` (one instance per process)

**Response models (Rust):**

```rust
// Summary response (deserialized directly):
struct ProtonDbSummaryResponse {
    tier: ProtonDbTier,
    best_reported_tier: Option<ProtonDbTier>,
    trending_tier: Option<ProtonDbTier>,
    score: Option<f32>,
    confidence: Option<String>,
    total_reports: Option<u32>,  // renamed from "total"
}

// Report feed entry:
struct ProtonDbReportEntry {
    id: String,
    timestamp: i64,
    responses: ProtonDbReportResponses {
        concluding_notes: String,
        launch_options: String,    // raw launch string e.g. "DXVK_ASYNC=1 %command%"
        proton_version: String,
        variant: String,
        notes: ProtonDbReportNotes { variant: String },
    }
}
```

## Database Schema

**Schema version:** 16 (as of latest migration)

**`external_cache_entries` table (migration 3→4):**

```sql
cache_id     TEXT PRIMARY KEY,
source_url   TEXT NOT NULL,
cache_key    TEXT NOT NULL UNIQUE,
payload_json TEXT,              -- NULL if payload exceeded MAX_CACHE_PAYLOAD_BYTES
payload_size INTEGER NOT NULL DEFAULT 0,
fetched_at   TEXT NOT NULL,
expires_at   TEXT,              -- NULL means no expiry; RFC3339 when set
created_at   TEXT NOT NULL,
updated_at   TEXT NOT NULL
```

Used by ProtonDB client, SteamGridDB, Steam metadata. `cache_key` format: `"{namespace}:{id}"`.

**`optimization_catalog` table (migration 11→12):**

```sql
sort_order              INTEGER NOT NULL,
id                      TEXT PRIMARY KEY,
applies_to_method       TEXT NOT NULL DEFAULT 'proton_run',
env_json                TEXT NOT NULL DEFAULT '[]',   -- JSON array of [key, value] pairs
wrappers_json           TEXT NOT NULL DEFAULT '[]',
conflicts_with_json     TEXT NOT NULL DEFAULT '[]',
required_binary         TEXT NOT NULL DEFAULT '',
label                   TEXT NOT NULL DEFAULT '',
description             TEXT NOT NULL DEFAULT '',
help_text               TEXT NOT NULL DEFAULT '',
category                TEXT NOT NULL DEFAULT '',
target_gpu_vendor       TEXT NOT NULL DEFAULT '',
advanced                INTEGER NOT NULL DEFAULT 0,
community               INTEGER NOT NULL DEFAULT 0,   -- 1 = came from a tap
applicable_methods_json TEXT NOT NULL DEFAULT '[]',
source                  TEXT NOT NULL DEFAULT 'default',
catalog_version         INTEGER NOT NULL DEFAULT 1,
updated_at              TEXT NOT NULL
```

**`profiles` table (migration 0→1 + subsequent ALTER TABLEs):**

```sql
profile_id          TEXT PRIMARY KEY,
current_filename    TEXT NOT NULL UNIQUE,
current_path        TEXT NOT NULL,
game_name           TEXT,
launch_method       TEXT,
content_hash        TEXT,
is_favorite         INTEGER NOT NULL DEFAULT 0,
source_profile_id   TEXT REFERENCES profiles(profile_id),
deleted_at          TEXT,
source              TEXT,
created_at          TEXT NOT NULL,
updated_at          TEXT NOT NULL
```

**`config_revisions` table (migration 10→11):**

```sql
id                    INTEGER PRIMARY KEY AUTOINCREMENT,
profile_id            TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
profile_name_at_write TEXT NOT NULL,
source                TEXT NOT NULL,      -- ConfigRevisionSource serialized
content_hash          TEXT NOT NULL,      -- SHA-256 of snapshot_toml
snapshot_toml         TEXT NOT NULL,
source_revision_id    INTEGER REFERENCES config_revisions(id),
is_last_known_working INTEGER NOT NULL DEFAULT 0,
created_at            TEXT NOT NULL
```

Trigger `trg_config_revisions_lineage_ownership` prevents `source_revision_id` crossing profile boundaries.

**`bundled_optimization_presets` table (migration 9→10):**

```sql
preset_id       TEXT PRIMARY KEY,
display_name    TEXT NOT NULL,
vendor          TEXT NOT NULL,     -- "nvidia" | "amd"
mode            TEXT NOT NULL,     -- "performance" | "quality"
option_ids_json TEXT NOT NULL,     -- JSON array of optimization entry IDs
catalog_version INTEGER NOT NULL DEFAULT 1,
created_at      TEXT NOT NULL
```

Seeded with 4 presets at migration time: `nvidia_performance`, `nvidia_quality`, `amd_performance`, `amd_quality`.

## Profile Data Model

**`GameProfile` struct** (`profile/models.rs`):

```rust
pub struct GameProfile {
    pub game: GameSection,
    pub trainer: TrainerSection,
    pub injection: InjectionSection,
    pub steam: SteamSection,
    pub runtime: RuntimeSection,
    pub launch: LaunchSection,
    pub local_override: LocalOverrideSection,
}
```

**`LaunchSection`** — primary target for config suggestions:

```rust
pub struct LaunchSection {
    pub method: String,
    pub optimizations: LaunchOptimizationsSection,   // active enabled_option_ids
    pub presets: BTreeMap<String, LaunchOptimizationsSection>, // named presets
    pub active_preset: String,
    pub custom_env_vars: BTreeMap<String, String>,   // KEY → VALUE map
    pub network_isolation: bool,                      // default true
    pub gamescope: GamescopeConfig,
    pub trainer_gamescope: GamescopeConfig,
    pub mangohud: MangoHudConfig,
}
```

**Env var application point:** `launch.custom_env_vars` — applied at launch after optimizations, lower priority than optimization-sourced vars.

**App ID resolution** (`resolve_art_app_id()`): prefers `steam.app_id` over `runtime.steam_app_id` (media-only field). The ProtonDB lookup uses the resolved art app ID.

## Optimization Catalog

**`OptimizationEntry` struct** (`launch/catalog.rs`):

```rust
pub struct OptimizationEntry {
    pub id: String,
    pub applies_to_method: String,   // "proton_run" default
    pub env: Vec<[String; 2]>,       // [[key, value], ...] pairs
    pub wrappers: Vec<String>,
    pub conflicts_with: Vec<String>,
    pub required_binary: String,
    pub label: String,
    pub description: String,
    pub help_text: String,
    pub category: String,            // one of: input, performance, display, graphics, compatibility
    pub target_gpu_vendor: String,   // empty = all vendors, or "nvidia"/"amd"
    pub advanced: bool,
    pub community: bool,
    pub applicable_methods: Vec<String>,
}
```

**Default catalog:** 25 entries embedded at compile time in `assets/default_optimization_catalog.toml`. Categories breakdown and full IDs available via `get_optimization_catalog` IPC command.

**Catalog merge order:** `embedded default → community taps (community=true forced) → user override file`. User override at `~/.config/crosshook/optimization_catalog.toml` wins on ID conflicts.

**Key catalog IDs from bundled presets:**

- Performance: `use_gamemode`, `enable_nvapi`, `enable_nvidia_libs`, `enable_dxvk_async`, `use_ntsync`, `force_large_address_aware`, `enable_fsr4_upgrade`
- Quality: `enable_dlss_upgrade`, `show_dlss_indicator`, `enable_hdr`, `enable_local_shader_cache`, `enable_xess_upgrade`, `enable_vkd3d_dxr`

## Env Var Safety Layer

**Location:** `aggregation.rs` — filtering occurs during ProtonDB report normalization, before any data reaches the cache or frontend.

**`is_safe_env_key(key: &str) → bool`:**

- First character must be `_` or `[A-Z]`
- Remaining characters: only `[A-Z0-9_]`
- Rejects lowercase, hyphens, dots, numbers as first character

**`is_safe_env_value(value: &str) → bool`:**

- No null bytes
- No whitespace characters
- No shell metacharacters: `$ ; " ' \ \` | & < > ( ) %`

**`RESERVED_ENV_KEYS` blocklist** (`aggregation.rs`):

```rust
const RESERVED_ENV_KEYS: &[&str] = &[
    "WINEPREFIX",
    "STEAM_COMPAT_DATA_PATH",
    "STEAM_COMPAT_CLIENT_INSTALL_PATH",
];
// Also blocks: anything starting with "STEAM_COMPAT_"
```

**`WINE_ENV_VARS_TO_CLEAR`** (`launch/env.rs`): 31 entries cleared before trainer launch — these are implicitly unsafe for user-set env vars too (e.g. `WINEDLLOVERRIDES`, `LD_PRELOAD`). The safety filter in aggregation does not explicitly check this list, but most of these would be blocked by the lowercase/non-`[A-Z_]` character rules.

**`BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS`** (`launch/env.rs`): 22 keys used by the default optimization catalog (e.g. `DXVK_ASYNC`, `PROTON_NO_STEAMINPUT`). ProtonDB suggestions may produce keys that overlap with this set — the implementor must decide merge priority.

## Tauri IPC Commands

**Existing ProtonDB commands:**

```rust
// src-tauri/src/commands/protondb.rs
#[tauri::command]
pub async fn protondb_lookup(
    app_id: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbLookupResult, String>
```

**Existing Profile commands (relevant to feature):**

```rust
// src-tauri/src/commands/profile.rs
profile_load(name) → GameProfile
profile_save(name, data: GameProfile) → ()
profile_save_launch_optimizations(name, optimizations: LaunchOptimizationsPayload) → ()
profile_list_bundled_optimization_presets() → Vec<BundledOptimizationPresetDto>
profile_apply_bundled_optimization_preset(name, preset_id) → GameProfile
profile_save_manual_optimization_preset(name, preset_name, enabled_option_ids) → GameProfile
```

**`LaunchOptimizationsPayload`** (IPC struct for optimization toggling):

```rust
pub struct LaunchOptimizationsPayload {
    pub enabled_option_ids: Vec<String>,
    pub switch_active_preset: Option<String>,
}
```

**Optimization catalog IPC:**

```rust
// src-tauri/src/commands/catalog.rs
#[tauri::command]
pub fn get_optimization_catalog() -> OptimizationCatalogPayload
```

## Caching Strategy

**TTL:** 6 hours (`CACHE_TTL_HOURS = 6` in `client.rs`). Stored as RFC3339 `expires_at` in `external_cache_entries`.

**Cache key:** `"protondb:{app_id}"` (`PROTONDB_CACHE_NAMESPACE = "protondb"`).

**Freshness check (non-stale load):**

```sql
SELECT payload_json, fetched_at, expires_at
FROM external_cache_entries
WHERE cache_key = ?1 AND (expires_at IS NULL OR expires_at > ?2)
```

**Stale load (fallback after network error):**

```sql
SELECT payload_json, fetched_at, expires_at
FROM external_cache_entries
WHERE cache_key = ?1
```

No `expires_at` check — any cached row is acceptable as fallback.

**Offline behavior:** `lookup_protondb()` never returns an `Err`. On network failure + no cache → `ProtonDbLookupState::Unavailable`. On network failure + stale cache → `ProtonDbLookupState::Stale` with `is_offline=true`.

**Eviction:** `evict_expired_cache_entries()` runs a DELETE for rows with `expires_at < now`. Called by startup sweep, not on every lookup.

**Max payload:** `MAX_CACHE_PAYLOAD_BYTES` (defined in `metadata/models.rs`). Payloads exceeding this store NULL in `payload_json`, and the cache entry effectively becomes a miss on the next load.

## Gotchas & Edge Cases

- **ProtonDB env vars may overlap with optimization catalog env vars.** `DXVK_ASYNC=1` may appear in both a ProtonDB suggestion and an enabled optimization entry. The `custom_env_vars` map is applied after optimization vars at launch, so a user-set env var from a ProtonDB suggestion may be silently overridden by an active optimization. The implementor must either detect and warn on this overlap or document the precedence explicitly.
- **Env keys from ProtonDB that include lowercase.** The `is_safe_env_key()` guard requires all-uppercase (after the first char). Community reports often contain lowercase env var keys like `mangohud=1` — these are silently dropped and never surface as suggestions.
- **Catalog `allowed_env_keys` is a compile-time derived set.** Any new env key accepted by a ProtonDB suggestion that is not in a catalog entry will NOT be in `allowed_env_keys`. This set is used by the runtime launch code for env-var filtering, not by the suggestion-apply path — but if new validation is added that gates on `allowed_env_keys`, it will reject ProtonDB-suggested keys not in the catalog.
- **`capture_config_revision` is content-hash-deduped.** If a user applies ProtonDB suggestions that result in no change to the TOML (e.g., vars already present with the same values), no new revision row is written. This is correct behavior but means the revision history will not show a "suggestion applied" event if the profile was already in the suggested state.
- **Stale cache is served as `Stale` state, not `Unavailable`.** The frontend hook `useProtonDbLookup` exposes `isStale` separately from `isUnavailable`. Any suggestion UI must handle the stale case gracefully — suggestions from a 6+ hour old cache may reference Proton versions no longer relevant.
- **`force_refresh` bypasses the cache read but not the cache write.** Even with `force_refresh=true`, a successful live fetch upserts the cache. A failed live fetch with `force_refresh=true` will still fall back to stale cache.
- **`switch_active_preset` in `LaunchOptimizationsPayload` takes precedence over `enabled_option_ids`.** If a suggestion-apply path passes both, the preset selection wins and the explicit IDs are ignored. Implementors should pass only one or the other.
- **Config revision `snapshot_toml` max size** is bounded by `MAX_SNAPSHOT_TOML_BYTES`. Large profiles with many env vars or presets could theoretically approach this limit, though in practice profiles are small.
- **`profile_save` auto-applies creation defaults** on first save (`is_new` check). If the suggestion-apply path is a first save, it will also apply settings-defined defaults. This is usually desirable but the caller should be aware.

## Licensing Constraint (ODbL)

ProtonDB data is licensed under the **Open Database License (ODbL)**. This carries a hard requirement:

- Cached ProtonDB data (suggestions, report text, tier values) **must NOT appear in profile exports** or community tap distributions.
- Suggestions must be applied as profile-owned values (in `custom_env_vars` or `enabled_option_ids`) — the resulting profile field contains CrossHook data, not ProtonDB data, and is safe to export.
- The `external_cache_entries` table payload must never be forwarded externally.
- The `source_url` in `external_cache_entries` (set to the ProtonDB page URL) satisfies attribution — do not strip it.

This constraint is already respected by the existing integration (suggestions are extracted and transformed at aggregation time, not forwarded verbatim). The implementor must maintain this boundary in any new apply/export path.

## Proposed Schema Addition (v17)

The docs-researcher identified a need for a `suggestion_dismissals` table to allow users to permanently or temporarily dismiss specific ProtonDB suggestions on a per-profile basis. This is a **new migration** (17) not yet in the codebase.

```sql
-- Proposed migration_16_to_17
CREATE TABLE IF NOT EXISTS suggestion_dismissals (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id     TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    app_id         TEXT NOT NULL,
    suggestion_key TEXT NOT NULL,  -- stable key identifying the suggestion (e.g. "env:DXVK_ASYNC=1")
    dismissed_at   TEXT NOT NULL,
    expires_at     TEXT NOT NULL   -- dismissals are time-bounded, not permanent
);
CREATE INDEX IF NOT EXISTS idx_suggestion_dismissals_profile_app
    ON suggestion_dismissals(profile_id, app_id);
```

**Design decisions to confirm during implementation:**

- What constitutes a `suggestion_key`? The docs-researcher suggests `"env:{KEY}={VALUE}"` for env vars and `"opt:{catalog_id}"` for optimization suggestions. This must be stable across ProtonDB refreshes.
- Should dismissals be per-app-version or per-app-id? (Reports change as game patches land.)
- Should `expires_at` be user-configurable or fixed (e.g. 30 days)?
- Migration must follow the existing migration pattern in `migrations.rs` — add a new `migrate_16_to_17()` function and chain it in `run_migrations()`.

## Proposed New Tauri IPC Commands

Three new commands identified by the docs-researcher. None exist yet — these are the implementation targets.

**1. `protondb_get_suggestions`** — enriched suggestion set with dismissal filtering applied server-side:

```rust
// Proposed command signature
#[tauri::command]
pub async fn protondb_get_suggestions(
    app_id: String,
    profile_name: String,
    force_refresh: Option<bool>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ProtonDbSuggestionSet, String>

// ProtonDbSuggestionSet would expose:
// - raw ProtonDbLookupResult (or just the snapshot)
// - filtered env_var_suggestions: Vec<ProtonDbEnvVarSuggestion> (dismissals excluded)
// - matched_catalog_suggestions: Vec<CatalogSuggestion> (ProtonDB → catalog ID bridge)
// - dismissed_keys: Vec<String> (for UI to know what was filtered)
```

**2. `protondb_accept_suggestion`** — atomic apply of one suggestion (env var or optimization ID) with re-validation at write time:

```rust
// Proposed command signature
#[tauri::command]
pub fn protondb_accept_suggestion(
    request: AcceptSuggestionRequest,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
    app: AppHandle,
) -> Result<AcceptSuggestionResult, String>

// AcceptSuggestionRequest:
//   profile_name: String
//   app_id: String
//   env_vars: Vec<EnvVarAcceptance>  // {key, value, overwrite_existing}
//   optimization_ids: Vec<String>
//
// AcceptSuggestionResult:
//   profile: GameProfile   // updated profile after apply
//   applied_env_vars: Vec<String>
//   applied_optimization_ids: Vec<String>
//   skipped: Vec<String>   // keys skipped due to conflict or validation failure
```

Must call `observe_profile_write()` + `capture_config_revision()` with a new source variant.

**3. `protondb_dismiss_suggestion`** — record a dismissal in `suggestion_dismissals`:

```rust
#[tauri::command]
pub fn protondb_dismiss_suggestion(
    profile_name: String,
    app_id: String,
    suggestion_key: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String>
```

**Required new `ConfigRevisionSource` variant:**

```rust
// In crosshook-core/src/metadata/models.rs (or wherever ConfigRevisionSource is defined)
ProtonDbSuggestionApply,  // apply path for community config suggestions
```

## Security Notes (from cross-team review)

**RESERVED_ENV_KEYS expansion (PREREQUISITE — ship before accept/apply flow):**

The docs-researcher (citing security review finding S2) flags that the current `RESERVED_ENV_KEYS` blocklist in `aggregation.rs` is incomplete. The following keys/prefixes are absent and must be added before any suggestion-apply flow ships:

```rust
// Current blocklist (aggregation.rs):
const RESERVED_ENV_KEYS: &[&str] = &[
    "WINEPREFIX",
    "STEAM_COMPAT_DATA_PATH",
    "STEAM_COMPAT_CLIENT_INSTALL_PATH",
];
// + dynamic: anything starting with "STEAM_COMPAT_"

// Required additions (security finding S2):
// Absolute blocks: "LD_PRELOAD", "LD_LIBRARY_PATH", "LD_AUDIT", "PATH", "HOME"
// Prefix blocks: "LD_*", "DYLD_*"
```

These keys can be used for library injection and privilege escalation. Since `is_safe_env_key()` already rejects lowercase characters, `LD_PRELOAD` and similar keys would only appear in suggestions if a ProtonDB report used them in uppercase. The guard does exist for lowercase variants but not for uppercase `LD_PRELOAD` — it passes the key regex check (`[A-Z_][A-Z0-9_]*`) and would reach the blocklist check without being rejected. This is a confirmed gap.

**Where to fix:** `aggregation.rs:safe_env_var_suggestions()` — expand `RESERVED_ENV_KEYS` and add a prefix check for `LD_` and `DYLD_` alongside the existing `STEAM_COMPAT_` prefix check.

**No new crate dependencies required.** All required crates (`reqwest`, `serde`/`serde_json`, `rusqlite`, `chrono`, `tokio`) are already present in `Cargo.toml`.
