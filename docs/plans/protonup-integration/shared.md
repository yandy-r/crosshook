# ProtonUp Integration

CrossHook is a Tauri v2 native Linux desktop app with a strict three-layer architecture: `crosshook-core` (Rust crate) owns all business logic, `src-tauri/src/commands/` provides thin `#[tauri::command]` IPC wrappers, and a React 18 + TypeScript frontend consumes commands via typed hooks and `invoke()`. The protonup-integration feature adds Proton/Wine version management (list available releases, install with progress streaming, delete) by consuming `libprotonup = "0.11.0"` (already declared but unused in `crosshook-core/Cargo.toml`), caching GitHub release data in the existing `external_cache_entries` SQLite table (schema v18), and surfacing a management panel in the Settings UI following the established `PrefixDepsPanel` + `prefix_deps.rs` streaming-install pattern. A GPL-3.0 licensing blocker with `libprotonup` (CrossHook is MIT) must be resolved before any code linking the library ships; Option B (direct `reqwest`/`flate2`/`tar`, all MIT-clean and already in `Cargo.toml`) is the fallback.

## Relevant Files

### Core Crate (crosshook-core)

- src/crosshook-native/crates/crosshook-core/Cargo.toml: Declares `libprotonup = "0.11.0"`, `reqwest`, `sha2`, `nix`, `rusqlite`, `tokio` -- all needed, no new deps required
- src/crosshook-native/crates/crosshook-core/src/lib.rs: Module registry -- new `pub mod protonup;` declaration goes here
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: `discover_compat_tools()` / `discover_compat_tools_with_roots()` -- post-install scan to refresh Proton list; must NOT be replaced by libprotonup's simpler scanner
- src/crosshook-native/crates/crosshook-core/src/steam/mod.rs: Exports `ProtonInstall`, `discover_compat_tools`, `discover_steam_root_candidates` -- types consumed by existing `list_proton_installs` command and new protonup feature
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs: `ProtonInstall` type with `name`, `path`, `aliases`, `is_official` fields
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: `MetadataStore` (`Arc<Mutex<Connection>>`) -- provides `get_cache_entry`/`put_cache_entry` for TTL cache; inject as `State<'_, MetadataStore>`
- src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs: Cache read/write/evict primitives; `MAX_CACHE_PAYLOAD_BYTES` enforced here -- verify release JSON fits
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Schema v18; `external_cache_entries` table from migration 3-to-4; new table would be v19 if needed
- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs: SQLite connection with WAL mode, FK enforcement
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `AppSettingsData` (TOML-backed) -- `default_proton_path` field exists; add `protonup_install_path` if user-configurable install dir needed
- src/crosshook-native/crates/crosshook-core/src/protondb/client.rs: **Reference pattern** -- `OnceLock<reqwest::Client>` singleton, TTL cache via `external_cache_entries`, stale fallback on network error, read-release-async-reacquire Mutex pattern
- src/crosshook-native/crates/crosshook-core/src/install/models.rs: **Reference pattern** -- Request/Result/Error triple with `#[serde(rename_all = "snake_case")]` and `From<ValidationError>` impl
- src/crosshook-native/crates/crosshook-core/src/install/service.rs: **Reference pattern** -- Validate-then-Execute (`validate_install_request` before `install_game`)
- src/crosshook-native/crates/crosshook-core/src/install/mod.rs: **Reference pattern** -- Module-per-domain with `mod.rs` re-exports
- src/crosshook-native/crates/crosshook-core/src/update/models.rs: **Reference pattern** -- Error enum with `.message()` method and exhaustive `Display` match

### Tauri IPC Layer (src-tauri)

- src/crosshook-native/src-tauri/src/lib.rs: Tauri `Builder` -- register new commands in `invoke_handler!`, manage new `ProtonUpInstallState`
- src/crosshook-native/src-tauri/src/commands/mod.rs: Command module declarations -- add `pub mod protonup;`
- src/crosshook-native/src-tauri/src/commands/steam.rs: `list_proton_installs` -- existing installed-versions source; protonup calls `reload()` on frontend hook after install
- src/crosshook-native/src-tauri/src/commands/prefix_deps.rs: **Closest structural parallel** -- background async install with `AppHandle::emit` progress streaming, managed state lock (`PrefixDepsInstallState`), `AppHandle + State` injection
- src/crosshook-native/src-tauri/src/commands/update.rs: **Reference pattern** -- long-running process with event-based streaming (`update-log`, `update-complete` events), `UpdateProcessState` cancellation pattern
- src/crosshook-native/src-tauri/src/commands/install.rs: **Reference pattern** -- thin command wrapper calling core service with `.map_err(|e| e.to_string())`

### Frontend (React/TypeScript)

- src/crosshook-native/src/hooks/useProtonInstalls.ts: Installed Proton versions hook -- must expose `reload()` for post-install refresh
- src/crosshook-native/src/hooks/useUpdateGame.ts: **Reference pattern** -- listen before invoke, `unlistenRef`, stage machine, `canStart`/`isRunning` derived state
- src/crosshook-native/src/types/proton.ts: Currently `ProtonInstallOption` only -- new `ProtonUpRelease`, `ProtonUpInstallProgress` types go here or in new `types/protonup.ts`
- src/crosshook-native/src/types/update.ts: **Reference pattern** -- TS types mirroring Rust structs with `snake_case` keys
- src/crosshook-native/src/types/settings.ts: `AppSettingsData`/`SettingsSaveRequest` frontend mirrors -- update when Rust settings change
- src/crosshook-native/src/components/SettingsPanel.tsx: 49k file, primary settings UI -- most likely home for ProtonUp management section (sub-panel like `PrefixDepsPanel`)
- src/crosshook-native/src/components/PrefixDepsPanel.tsx: **Closest UI parallel** -- background install with live log streaming, managed install lock, progress feedback
- src/crosshook-native/src/context/PreferencesContext.tsx: App-wide settings context -- `settings.default_proton_path` updated on install if empty
- src/crosshook-native/src/components/layout/Sidebar.tsx: `AppRoute` union type + `SIDEBAR_SECTIONS` -- embedding in Settings avoids new top-level route
- src/crosshook-native/src/hooks/useScrollEnhance.ts: **CRITICAL** -- any new `overflow-y: auto` container MUST be added to `SCROLLABLE` selector

## Relevant Tables

- external_cache_entries: TTL-cached data store (cache_key TEXT UNIQUE, payload_json TEXT, expires_at TEXT). Used for GitHub release list caching. Cache key pattern: `github:proton-releases:GEProton`. Access via `MetadataStore::get_cache_entry`/`put_cache_entry`.
- profiles: Game launch profiles referencing Proton paths. After installing a new version, `check_proton_migrations`/`apply_proton_migration` can update stale paths.

## Relevant Patterns

**Request/Result/Error Triple**: Every domain defines `*Request`, `*Result`, and `*Error` types in `models.rs`. Error enum derives `Serialize + Deserialize` with `#[serde(rename_all = "snake_case")]`, implements `Display` via `.message()`, and has `From<*ValidationError>`. See [src/crosshook-native/crates/crosshook-core/src/install/models.rs](src/crosshook-native/crates/crosshook-core/src/install/models.rs).

**Validate-then-Execute**: Core service functions begin with explicit `validate_*` call returning typed validation error before touching filesystem/network. See [src/crosshook-native/crates/crosshook-core/src/install/service.rs](src/crosshook-native/crates/crosshook-core/src/install/service.rs).

**Module-per-Domain**: Each domain is a directory with `mod.rs`, `models.rs`, `service.rs`, optionally `client.rs`/`discovery.rs`/`tests.rs`. Public surface re-exported from `mod.rs`. See [src/crosshook-native/crates/crosshook-core/src/install/mod.rs](src/crosshook-native/crates/crosshook-core/src/install/mod.rs).

**Thin Tauri Command Layer**: `#[tauri::command]` functions call core functions and `.map_err(|e| e.to_string())`. No business logic in command files. See [src/crosshook-native/src-tauri/src/commands/install.rs](src/crosshook-native/src-tauri/src/commands/install.rs).

**OnceLock HTTP Client Singleton**: Long-lived `reqwest::Client` in `static OnceLock<reqwest::Client>`, lazily initialized with `CrossHook/<version>` user-agent and timeout. See [src/crosshook-native/crates/crosshook-core/src/protondb/client.rs](src/crosshook-native/crates/crosshook-core/src/protondb/client.rs).

**AppHandle::emit Event Streaming**: Long-running commands return `Result` immediately, then emit named events for progress and completion. Frontend subscribes before `invoke`-ing. See [src/crosshook-native/src-tauri/src/commands/prefix_deps.rs](src/crosshook-native/src-tauri/src/commands/prefix_deps.rs) and [src/crosshook-native/src/hooks/useUpdateGame.ts](src/crosshook-native/src/hooks/useUpdateGame.ts).

**Managed Tauri State**: State objects constructed in `lib.rs`, registered via `.manage()`, accessed via `State<'_, T>` parameters. Mutable state uses `Mutex<Option<T>>`. See [src/crosshook-native/src-tauri/src/lib.rs](src/crosshook-native/src-tauri/src/lib.rs).

**MetadataStore Cache-First Fetch**: Read cache (acquiring lock briefly), release lock, do async network IO, reacquire lock to write cache. Never hold Mutex during async operations. See [src/crosshook-native/crates/crosshook-core/src/protondb/client.rs](src/crosshook-native/crates/crosshook-core/src/protondb/client.rs).

**Frontend Invoke Hook**: `useState` + `useEffect` + `invoke()` + `reload()` callback pattern. For streaming operations: listen before invoke, `unlistenRef`, stage machine with `canStart`/`isRunning`. See [src/crosshook-native/src/hooks/useProtonInstalls.ts](src/crosshook-native/src/hooks/useProtonInstalls.ts) and [src/crosshook-native/src/hooks/useUpdateGame.ts](src/crosshook-native/src/hooks/useUpdateGame.ts).

## Relevant Docs

**docs/plans/protonup-integration/feature-spec.md**: You _must_ read this as the authoritative implementation spec -- business rules (BR-1 through BR-13), data models, 5 Tauri command contracts, 3-phase task breakdown, resolved decisions.

**docs/plans/protonup-integration/research-security.md**: You _must_ read this before writing extraction or IPC code -- 3 CRITICAL findings (CVE chain in `astral-tokio-tar`, `install_dir` path traversal, archive bomb), required mitigations with verified code patterns.

**docs/plans/protonup-integration/research-external.md**: You _must_ read this for complete `libprotonup 0.11.0` API surface, GitHub Releases API spec, integration patterns, and resolved Q&A on pagination/rate limits.

**docs/plans/protonup-integration/research-recommendations.md**: You _must_ read this for implementation phasing, technology choices (libprotonup vs Option B), risk table, and GPL-3.0 blocker.

**docs/plans/protonup-integration/research-technical.md**: You _must_ read this during Phase 1 for Rust type definitions, API contracts, architecture decisions, and complete file create/modify list.

**docs/plans/protonup-integration/research-practices.md**: You _must_ read this for reusable module inventory with exact file paths and line numbers, KISS assessment, testability patterns, and the rule to copy (not abstract) the cache-first fetch pattern.

**docs/plans/protonup-integration/research-ux.md**: You _must_ read this during Phase 2/3 for component vocabulary, user flows, API-to-UX binding, and accessibility requirements.

**AGENTS.md**: You _must_ read this for binding architecture rules, IPC naming, directory map, and persistence classification.

**CLAUDE.md**: You _must_ read this for `useScrollEnhance` scroll registration, commit/PR conventions, `docs(internal):` prefix, and label taxonomy.
