# UI Enhancements — Profiles Page Restructuring + Game Metadata & Cover Art

CrossHook's Profiles page currently hides its entire editing surface behind a single collapsed `CollapsibleSection("Advanced", defaultOpen=false)` wrapping the 41k `ProfileFormSections` monolith; the restructuring replaces this with visually distinct section cards (Identity, Game Path, Runtime, Environment/ProtonDB, Trainer) using the existing `CollapsibleSection` + `crosshook-panel` card primitive, with Phase 3 adding `@radix-ui/react-tabs` sub-tab navigation using the already-defined `.crosshook-subtab-*` CSS classes. The game metadata and cover art integration (GitHub #52) mirrors the ProtonDB lookup end-to-end: a new `steam_metadata/` Rust module in `crosshook-core` with `OnceLock<reqwest::Client>`, cache-first lookup via `external_cache_entries` (key `steam:appdetails:v1:{app_id}`, 24h TTL), thin IPC commands in `src-tauri/src/commands/game_metadata.rs`, and a frontend `useGameMetadata` hook cloning the `useProtonDbLookup` state machine (`idle/loading/ready/stale/unavailable` + `requestIdRef` race guard). Cover art images are downloaded from Steam CDN, validated with the `infer` crate (magic-byte MIME check, SVG rejection), cached to `~/.local/share/crosshook/cache/images/{steam_app_id}/` tracked by a new `game_image_cache` SQLite table (schema v14 migration), and rendered via Tauri's asset protocol (`convertFileSrc` + CSP `img-src asset:` extension).

## Relevant Files

### Frontend — Restructuring Targets

- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Primary restructuring target (36k); wraps all fields in single collapsed Advanced section — remove outer CollapsibleSection, promote groups to individual cards
- src/crosshook-native/src/components/ProfileFormSections.tsx: 41k monolith holding all profile form fields; shared with InstallPage via `reviewMode` prop — must preserve this contract during extraction
- src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx: Holds local `rows` state; must use CSS `display: none` (not conditional rendering) to preserve state during tab switches
- src/crosshook-native/src/components/ProtonDbLookupCard.tsx: Standalone card wrapping useProtonDbLookup — template for new GameMetadataCard; must stay co-located with env vars section
- src/crosshook-native/src/components/ProfileActions.tsx: Save/Delete/Duplicate/Rename bar — must remain outside any collapsible or tab panel
- src/crosshook-native/src/components/pages/InstallPage.tsx: Consumes ProfileFormSections with `reviewMode={true}` — structural changes must be backward-compatible

### Frontend — UI Primitives and State

- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: `<details>`-based collapsible with `defaultOpen`, controlled `open/onToggle`, and `meta` slot for badges — the card container primitive
- src/crosshook-native/src/components/HealthBadge.tsx: Profile health badge rendered in CollapsibleSection meta slot — pattern for section summary badges
- src/crosshook-native/src/context/ProfileContext.tsx: Single source of truth for active profile state; provides `profile`, `updateProfile`, `dirty`, `saveProfile`, `launchMethod`
- src/crosshook-native/src/hooks/useProfile.ts: 46k hook owning profile CRUD via invoke(); `updateProfile(updater)` accepts immutable updater function
- src/crosshook-native/src/hooks/useProtonDbLookup.ts: Canonical external API hook pattern — requestIdRef race guard, stale-while-revalidating state machine, refresh() callback

### Frontend — Styles

- src/crosshook-native/src/styles/variables.css: CSS custom properties including `--crosshook-subtab-min-height` and `--crosshook-subtab-padding-inline` — sub-tab infrastructure already defined
- src/crosshook-native/src/styles/theme.css: 90k stylesheet; contains `.crosshook-subtab-row`, `.crosshook-subtab`, `.crosshook-subtab--active` classes (unused); new cover art and skeleton classes go here

### Frontend — Types

- src/crosshook-native/src/types/protondb.ts: ProtonDB TypeScript types mirroring Rust serde output — template for new steam_metadata types
- src/crosshook-native/src/types/settings.ts: Frontend AppSettingsData interface — must add `steamgriddb_api_key?: string | null` (Phase 3)

### Backend — ProtonDB Reference Pattern

- src/crosshook-native/crates/crosshook-core/src/protondb/client.rs: Cache-first external API client with OnceLock HTTP singleton, stale fallback, MetadataStore integration — the exact model for steam_metadata/client.rs
- src/crosshook-native/crates/crosshook-core/src/protondb/models.rs: ProtonDbLookupResult/State/Snapshot types with Serde annotations — mirror structure for Steam metadata types
- src/crosshook-native/crates/crosshook-core/src/protondb/tests.rs: Unit tests using MetadataStore::open_in_memory() with seeded cache entries — test pattern for new module

### Backend — MetadataStore and Schema

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: MetadataStore (Arc<Mutex<Connection>>); exposes put_cache_entry, get_cache_entry, with_sqlite_conn; graceful degradation via with_conn returning T::default()
- src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: get/put/evict for external_cache_entries; 512 KiB payload cap (MAX_CACHE_PAYLOAD_BYTES)
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs: Store submodule pattern — functions take `&Connection` directly, MetadataStore delegates — template for game_image_store.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential `if version < N` migration pattern; currently at schema v13; new v14 migration adds game_image_cache table
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: MetadataStoreError with Database { action, source } variant; MAX_CACHE_PAYLOAD_BYTES constant

### Backend — Settings

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: AppSettingsData with `#[serde(default)]` at struct level — adding optional fields requires no migration
- src/crosshook-native/crates/crosshook-core/Cargo.toml: reqwest + rusqlite already present; `infer ~0.16` is the one new dependency

### Tauri Layer

- src/crosshook-native/src-tauri/src/lib.rs: Entry point; .manage() for stores, invoke_handler! for command registration — new commands added here
- src/crosshook-native/src-tauri/src/commands/protondb.rs: Minimal 13-line command — the exact template for new game_metadata commands
- src/crosshook-native/src-tauri/src/commands/mod.rs: Module registry — new game_metadata module declared here
- src/crosshook-native/src-tauri/src/commands/steam.rs: Existing Steam commands (auto_populate, proton discovery) — new metadata commands go in separate game_metadata.rs
- src/crosshook-native/src-tauri/tauri.conf.json: CSP needs `img-src 'self' asset: http://asset.localhost` for cover art rendering
- src/crosshook-native/src-tauri/capabilities/default.json: Needs fs:allow-read-file scoped to `$LOCALDATA/crosshook/cache/images/**` and asset protocol scope

## Relevant Tables

- external_cache_entries: Stores Steam metadata JSON (cache_key `steam:appdetails:v1:{app_id}`, payload 3-15 KiB); upsert on cache_key conflict; 512 KiB cap
- game_image_cache (NEW, v14): Stores filesystem paths to cached cover art images; keyed by (steam_app_id, image_type, source); includes file_path, mime_type, content_hash, expires_at
- profiles: Profile registry with game_name, launch_method; steam_app_id is NOT in this table (lives in TOML profile files)
- version_snapshots: Contains steam_app_id column with index — existing pattern for steam_app_id in SQLite

## Relevant Patterns

**Thin IPC Command Layer**: Tauri commands do no business logic — clone MetadataStore, delegate to crosshook-core, return `Result<T, String>`. See [src/crosshook-native/src-tauri/src/commands/protondb.rs](src/crosshook-native/src-tauri/src/commands/protondb.rs) for the 13-line template.

**Cache-First with Stale Fallback**: Check valid cache → fetch live on miss → persist on success → load expired cache on network failure → return Unavailable on total failure. See [src/crosshook-native/crates/crosshook-core/src/protondb/client.rs](src/crosshook-native/crates/crosshook-core/src/protondb/client.rs) lines 85-130.

**OnceLock HTTP Client Singleton**: Per-module `OnceLock<reqwest::Client>` with timeout and user_agent. See [protondb/client.rs](src/crosshook-native/crates/crosshook-core/src/protondb/client.rs) line 26.

**MetadataStore Access**: `with_conn()` for graceful degradation (returns T::default()), `with_sqlite_conn()` for strict access. Store submodules take `&Connection` directly. See [metadata/mod.rs](src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs) and [health_store.rs](src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs).

**Sequential Migration**: `if version < N { migrate_N_minus_1_to_N(conn)?; pragma_update("user_version", N); }` — not else-if. See [migrations.rs](src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs).

**Frontend Hook State Machine**: `idle/loading/ready/stale/unavailable` with `requestIdRef` race guard, stale-while-revalidating, and `refresh()` callback. See [useProtonDbLookup.ts](src/crosshook-native/src/hooks/useProtonDbLookup.ts).

**CollapsibleSection Card**: `<CollapsibleSection title="..." className="crosshook-panel" meta={<Badge/>}>` — the card primitive. Use `defaultOpen` for expanded-by-default sections. See [CollapsibleSection.tsx](src/crosshook-native/src/components/ui/CollapsibleSection.tsx).

**CSS BEM with crosshook- prefix**: All classes use `crosshook-` prefix with BEM-like modifiers. Status chips use `crosshook-status-chip`. Variables defined in variables.css, consumed in theme.css.

**Serde IPC Types**: All types crossing IPC derive `Serialize + Deserialize`. Enums use `#[serde(rename_all = "snake_case")]`. Optional fields use `#[serde(default, skip_serializing_if)]`.

## Relevant Docs

**docs/plans/ui-enhancements/feature-spec.md**: You _must_ read this before writing any code. The authoritative implementation contract with data models, API signatures, file lists, phasing (Phase 0-4), risk assessment, and persistence classification.

**AGENTS.md**: You _must_ read this when working on any structural change. Hard constraints on architecture (`crosshook-core` owns logic, `src-tauri` is IPC-thin), persistence classification rules, 512 KiB cache cap, and directory map.

**docs/plans/ui-enhancements/research-security.md**: You _must_ read this when implementing image download, cache construction, or Tauri config changes. Contains code-ready Rust snippets for SVG rejection (`validate_image_bytes`) and path traversal prevention (`safe_image_cache_path`), plus exact JSON for CSP and capabilities config.

**docs/plans/ui-enhancements/research-technical.md**: You _must_ read this when working on Rust modules or React component decomposition. Full component tree, ProfileContext state flow, CSS pattern inventory.

**docs/plans/ui-enhancements/research-practices.md**: You _must_ read this before creating new components. Inventory of reusable existing components and CSS grid infrastructure to prevent duplication.

**docs/plans/protondb-lookup/research-technical.md**: You _must_ read this when implementing the steam_metadata Rust module. The exact pattern to mirror: module layout, cache-key naming, DTO structure.

**docs/plans/ui-enhancements/research-recommendations.md**: Reference for approach evaluation (cards vs. sub-tabs vs. hybrid) and phasing rationale.

**docs/plans/ui-enhancements/research-ux.md**: Reference for GameCoverArt, GameMetadataBar, and ProfileGameCard component implementations including shimmer skeleton and controller-mode requirements.

**CONTRIBUTING.md**: Reference for PR workflow, commit conventions, and build prerequisites.
