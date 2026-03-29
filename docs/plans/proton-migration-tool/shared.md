# Proton Migration Tool

The Proton migration tool extends CrossHook's existing health and Steam discovery infrastructure to detect stale Proton paths in profiles and suggest same-family replacements. The core architecture adds a new `steam/migration.rs` module that consumes `discover_compat_tools()` for Proton discovery and `normalize_alias()` for family-based fuzzy matching, then writes updated paths through the existing `ProfileStore::load()`/`save()` cycle which transparently handles the `local_override` layer. Three new Tauri IPC commands (`check_proton_migrations`, `apply_proton_migration`, `apply_batch_migration`) follow the existing command pattern with `State<'_, ProfileStore>` injection, and the Health Dashboard UI surfaces migration actions inline without requiring a new page.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: Proton discovery (`discover_compat_tools`), `normalize_alias()` (promote to `pub(crate)`), `resolve_compat_tool_by_name()` -- core dependencies for matching
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs: `ProtonInstall` struct with `name`, `path`, `aliases`, `normalized_aliases`, `is_official` -- the candidate data model
- src/crosshook-native/crates/crosshook-core/src/steam/mod.rs: Module re-exports -- add `pub mod migration;`
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` with `steam.proton_path`, `runtime.proton_path`, `local_override`, `effective_profile()`, `storage_profile()` -- migration target
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore::load()`, `save()`, `list()` -- profile CRUD used by migration
- src/crosshook-native/crates/crosshook-core/src/profile/health.rs: `check_profile_health()`, `batch_check_health()`, `HealthIssue` -- stale detection already exists here
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: `SyncSource` enum (add `AppMigration` variant) -- defined here, not in profile_sync.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs: `observe_profile_write()`, `created_at_for_insert()` exhaustive match on `SyncSource` -- add `AppMigration` arm
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs: Health snapshot persistence -- invalidate after migration
- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs: `discover_steam_root_candidates()` -- provides steam root paths for Proton discovery
- src/crosshook-native/src-tauri/src/commands/profile.rs: Existing profile save command pattern -- follow for migration commands
- src/crosshook-native/src-tauri/src/commands/steam.rs: `list_proton_installs`, `default_steam_client_install_path()` -- reuse for migration scan
- src/crosshook-native/src-tauri/src/commands/health.rs: `batch_validate_profiles` -- existing health IPC pattern to follow
- src/crosshook-native/src-tauri/src/commands/shared.rs: `sanitize_display_path()` -- apply to all migration IPC path results
- src/crosshook-native/src-tauri/src/commands/mod.rs: Command module registry -- add migration commands
- src/crosshook-native/src-tauri/src/lib.rs: Tauri `invoke_handler` -- register new migration commands
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx: Health Dashboard with `TableToolbar`, `categorizeIssue()`, issue row expansion -- migration UI integration point
- src/crosshook-native/src/components/HealthBadge.tsx: Health score badge -- reuse for migration status display
- src/crosshook-native/src/components/LauncherPreviewModal.tsx: Modal shell with portal, focus trap, ARIA, Tab cycling, Escape handler -- use as base for migration review modal
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: Collapsible section component -- use for "Show full path" expand
- src/crosshook-native/src/hooks/useProfileHealth.ts: `revalidateSingle()`, `batchValidate()` -- call after migration to refresh health
- src/crosshook-native/src/styles/variables.css: CSS custom properties including `--crosshook-color-warning`, `--crosshook-color-danger` -- use for confidence indicators
- src/crosshook-native/src/styles/focus.css: `.crosshook-focus-ring`, `.crosshook-nav-target` -- gamepad/controller focus classes
- src/crosshook-native/src/types/index.ts: Type re-exports -- add migration types

## Relevant Tables

- health_snapshots: Profile health scores keyed by `profile_id` -- invalidate rows for migrated profiles after successful write
- profile_name_history: Tracks profile renames -- model for future `migration_events` table (Phase 2)

## Relevant Patterns

**Tauri IPC Command Pattern**: All backend operations use `#[tauri::command]` with `State<'_, ProfileStore>` and `State<'_, MetadataStore>` injection. Commands return `Result<T, String>` with stringified errors. See [src/crosshook-native/src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs) for the standard pattern.

**Profile Load/Save Roundtrip**: `ProfileStore::load()` calls `effective_profile()` (merges local_override into base, clears override section). `ProfileStore::save()` calls `storage_profile()` (moves machine-local paths back to local_override). Migration uses this same cycle -- no special override handling needed. See [src/crosshook-native/crates/crosshook-core/src/profile/models.rs](src/crosshook-native/crates/crosshook-core/src/profile/models.rs) lines 243-298.

**Batch Health Check Pattern**: `batch_check_health()` iterates all profiles, collects per-profile results, and returns them without aborting on individual failures. Migration batch follows this same best-effort approach. See [src/crosshook-native/crates/crosshook-core/src/profile/health.rs](src/crosshook-native/crates/crosshook-core/src/profile/health.rs).

**Metadata Sync After Write**: Every profile mutation calls `metadata_store.observe_profile_write()` as a fail-soft (logged, not fatal) post-save step. See [src/crosshook-native/src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs) line ~110.

**Modal Shell Pattern**: `LauncherPreviewModal` provides a complete accessibility shell (portal, focus trap, Tab cycling, Escape, aria-modal, inert background, focus restore). New modals copy this shell and replace body content. See [src/crosshook-native/src/components/LauncherPreviewModal.tsx](src/crosshook-native/src/components/LauncherPreviewModal.tsx).

**Health Dashboard Issue Categorization**: `categorizeIssue()` maps `HealthIssue.field` to categories like `missing_proton`. Migration toolbar button triggers on this category count. See [src/crosshook-native/src/components/pages/HealthDashboardPage.tsx](src/crosshook-native/src/components/pages/HealthDashboardPage.tsx) lines 39-48.

## Relevant Docs

**docs/plans/proton-migration-tool/feature-spec.md**: You _must_ read this when working on any migration task. Contains business rules, data models, API contracts, UX specifications, and security requirements.

**docs/plans/proton-migration-tool/research-technical.md**: You _must_ read this when implementing the backend. Contains detailed Rust struct definitions, matching algorithm with code examples, Tauri command signatures, and the complete scan/apply flow.

**docs/plans/proton-migration-tool/research-business.md**: You _must_ read this when implementing business logic. Contains version suggestion tiers, edge cases, workflow diagrams, and the domain model.

**docs/plans/proton-migration-tool/research-ux.md**: You _must_ read this when implementing frontend components. Contains wireframe descriptions, confidence-level visual treatment, gamepad requirements, and competitive analysis.

**docs/plans/proton-migration-tool/research-security.md**: You _must_ read this when handling file writes or path validation. Contains 4 WARNING-level findings with required mitigations and secure coding guidelines.

**docs/plans/proton-migration-tool/research-practices.md**: You _must_ read this when deciding module boundaries or code reuse. Contains reuse inventory, KISS assessment, and interface design recommendations.
