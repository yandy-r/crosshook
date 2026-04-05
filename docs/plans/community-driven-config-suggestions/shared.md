# Community-Driven Configuration Suggestions

CrossHook's ProtonDB pipeline is already fully implemented end-to-end: `protondb/client.rs` fetches and caches reports with 6h TTL, `protondb/aggregation.rs` extracts env vars from `launch_options`, filters via `safe_env_var_suggestions()`, and frequency-ranks into `ProtonDbRecommendationGroup` structs exposed through a single `protondb_lookup` Tauri command. The frontend has `useProtonDbLookup` hook, `ProtonDbLookupCard` with a wired `onApplyEnvVars` callback, `mergeProtonDbEnvVarGroup()` conflict detection utility, and `ProtonDbOverwriteConfirmation` modal. The remaining work is: expanding the `RESERVED_ENV_KEYS` blocklist (security-critical S2), adding catalog-matching so ProtonDB-suggested env vars that match known optimization entries toggle `enabled_option_ids` instead of writing to `custom_env_vars`, adding a `ConfigRevisionSource` variant for suggestion-apply tracking, and deduplicating the dual Apply paths in `LaunchPage.tsx` and `ProfileFormSections.tsx` into a shared utility.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/protondb/client.rs: ProtonDB HTTP client with cache-read/live-fetch/stale-fallback pipeline
- src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs: Env var extraction, safety filtering (`is_safe_env_key`, `is_safe_env_value`), frequency-ranking, `RESERVED_ENV_KEYS` blocklist
- src/crosshook-native/crates/crosshook-core/src/protondb/models.rs: All IPC-crossing types: `ProtonDbLookupResult`, `ProtonDbRecommendationGroup`, `ProtonDbEnvVarSuggestion`
- src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs: Unit tests for aggregation and cache fallback
- src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs: `OptimizationCatalog`, `OptimizationEntry` with `env: Vec<[String; 2]>`, `global_catalog()` singleton
- src/crosshook-native/crates/crosshook-core/src/launch/env.rs: `RESERVED_ENV_KEYS`, `BUILTIN_LAUNCH_OPTIMIZATION_ENV_VARS` (22 keys), `WINE_ENV_VARS_TO_CLEAR` (31 keys)
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` with `launch.custom_env_vars: BTreeMap<String, String>` and `launch.optimizations.enabled_option_ids: Vec<String>`
- src/crosshook-native/crates/crosshook-core/src/metadata/config_history_store.rs: `capture_config_revision()`, `ConfigRevisionSource` enum (needs new variant)
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: SQLite schema migrations (current version needs verification)
- src/crosshook-native/assets/default_optimization_catalog.toml: 25 built-in optimization entries with env key-value pairs
- src/crosshook-native/src-tauri/src/commands/protondb.rs: Thin `protondb_lookup(app_id, force_refresh)` Tauri command
- src/crosshook-native/src-tauri/src/commands/profile.rs: `profile_save()`, `profile_save_launch_optimizations()`, config revision tracking
- src/crosshook-native/src-tauri/src/commands/catalog.rs: `get_optimization_catalog()` returning full catalog payload
- src/crosshook-native/src-tauri/src/lib.rs: `invoke_handler!` macro — all Tauri command registration
- src/crosshook-native/src/hooks/useProtonDbLookup.ts: Race-safe hook wrapping `protondb_lookup` IPC with `requestIdRef` pattern
- src/crosshook-native/src/utils/protondb.ts: `mergeProtonDbEnvVarGroup()` conflict detection — returns `{ mergedEnvVars, conflicts, appliedKeys, unchangedKeys }`
- src/crosshook-native/src/utils/optimization-catalog.ts: `fetchOptimizationCatalog()`, `buildOptionsById()`, `buildConflictMatrix()`
- src/crosshook-native/src/components/ProtonDbLookupCard.tsx: Renders recommendation groups; `onApplyEnvVars` callback wired but catalog-matching path not implemented
- src/crosshook-native/src/components/ProtonDbOverwriteConfirmation.tsx: Per-key conflict resolution modal
- src/crosshook-native/src/components/pages/LaunchPage.tsx: Orchestrates Apply flow — `applyProtonDbGroup()` writes to `custom_env_vars` only
- src/crosshook-native/src/components/ProfileFormSections.tsx: Duplicates Apply flow for profile form context — same catalog-matching gap
- src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx: Renders optimization toggles from catalog; `onToggleOption` write path for `enabled_option_ids`
- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: Row-based env var editor with `RESERVED_CUSTOM_ENV_KEYS` Set (must mirror Rust blocklist)
- src/crosshook-native/src/types/protondb.ts: TypeScript mirror of Rust ProtonDB models
- src/crosshook-native/src/types/profile.ts: `GameProfile` TS type with `launch.custom_env_vars` and `launch.optimizations.enabled_option_ids`
- src/crosshook-native/src/hooks/useProfile.ts: Central `updateProfile(updater)` immutable-spread pattern
- src/crosshook-native/src/styles/variables.css: CSS custom properties including ProtonDB-specific tokens
- src/crosshook-native/src/styles/theme.css: BEM component styles with `crosshook-` prefix
- src/crosshook-native/src/hooks/useScrollEnhance.ts: `SCROLLABLE` selector — new scroll containers must register here

## Relevant Tables

- external_cache_entries: ProtonDB cache with `cache_key="protondb:{app_id}"`, `payload_json`, `expires_at` (6h TTL), stale-while-revalidate fallback
- profiles: Profile metadata linking `profile_id` to TOML files
- config_revisions: Content-hash-deduped revision history with `source` (ConfigRevisionSource), `snapshot_toml`
- optimization_catalog: 25+ entries with `id`, `env_json` (key-value pairs), `category`, `community` flag
- bundled_optimization_presets: 4 presets (nvidia/amd x performance/quality) with `option_ids_json`

## Relevant Patterns

**Immutable Profile Update**: All profile mutations go through `updateProfile(updater: (current) => GameProfile)` with spread-copy. See [src/crosshook-native/src/hooks/useProfile.ts](src/crosshook-native/src/hooks/useProfile.ts).

**Race-Safe IPC Hook**: Every Tauri IPC call is encapsulated in a hook with `requestIdRef` counter to discard stale responses. See [src/crosshook-native/src/hooks/useProtonDbLookup.ts](src/crosshook-native/src/hooks/useProtonDbLookup.ts).

**Thin Tauri Command Layer**: Commands receive `State<'_, Store>`, call one `crosshook_core` function, map errors to `String`. See [src/crosshook-native/src-tauri/src/commands/protondb.rs](src/crosshook-native/src-tauri/src/commands/protondb.rs).

**Aggregation-Time Safety Filtering**: Env vars are filtered by `safe_env_var_suggestions()` before caching — frontend receives only pre-validated key-value pairs. See [src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs](src/crosshook-native/crates/crosshook-core/src/protondb/aggregation.rs).

**Conflict-Aware Merge Utility**: `mergeProtonDbEnvVarGroup()` computes `conflicts`, `appliedKeys`, `unchangedKeys` for per-key resolution. See [src/crosshook-native/src/utils/protondb.ts](src/crosshook-native/src/utils/protondb.ts).

**Pending State for Confirmation Flows**: Multi-step confirmation uses `useState<PendingType | null>(null)` — non-null renders a modal; confirm/cancel resets to null. See [src/crosshook-native/src/components/ProfileFormSections.tsx](src/crosshook-native/src/components/ProfileFormSections.tsx).

**BEM CSS with `crosshook-` Prefix**: Variables in `variables.css`, component styles in `theme.css` using `crosshook-<component>__<element>--<modifier>`. See [src/crosshook-native/src/styles/theme.css](src/crosshook-native/src/styles/theme.css).

**Config Revision Tracking**: Every profile write calls `observe_profile_write()` + `capture_config_revision()` with a `ConfigRevisionSource` variant. Content-hash-deduped. See [src/crosshook-native/src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs).

## Relevant Docs

**docs/plans/community-driven-config-suggestions/feature-spec.md**: You _must_ read this when implementing any part of this feature — consolidated spec with all decisions, user stories, business rules, data models, API design, security requirements, and phasing.

**docs/plans/community-driven-config-suggestions/research-security.md**: You _must_ read this before implementing any accept/apply flow — S2 (`LD_PRELOAD`, `PATH`, `HOME`, `LD_*` prefix blocklist expansion) is CRITICAL and must ship before any apply flow.

**AGENTS.md**: You _must_ read this when adding new Tauri commands, modifying SQLite schema, or adding new scroll containers — architecture rules, directory map, schema inventory.

**docs/plans/community-driven-config-suggestions/research-technical.md**: You _must_ read this when implementing Rust backend changes — data models, Tauri command signatures, three-tier suggestion architecture, files to create/modify.

**docs/plans/community-driven-config-suggestions/research-practices.md**: You _must_ read this when implementing frontend wiring — existing reusable code inventory, `OnboardingWizard.tsx` step 3 integration point, golden test fixture patterns.

**docs/plans/community-driven-config-suggestions/research-ux.md**: Reference for UI/UX decisions — all user workflows, UI anatomy, confidence display, dismissal patterns, competitive analysis.

**docs/plans/community-driven-config-suggestions/research-business.md**: Reference for domain logic — storage boundary classification table, all business rules (BR-1 through BR-16), edge cases (EC-1 through EC-11).
