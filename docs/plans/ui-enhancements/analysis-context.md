# Context Analysis: ui-enhancements

## Executive Summary

This feature restructures the Profiles page from a single collapsed "Advanced" `<details>` wrapper hiding all editing fields into visually distinct section cards, then layers on Steam Store API game metadata and cover art fetching (GitHub #52). The architectural approach mirrors the existing ProtonDB lookup end-to-end — cache-first via `external_cache_entries`, stale fallback, `MetadataStore` as the single SQLite access point — with image binaries going to filesystem + a new `game_image_cache` SQLite table (schema v14) because they exceed the 512 KiB JSON cache cap.

## Architecture Context

- **System Structure**: Tauri v2 native Linux app. `crosshook-core` owns all business logic; `src-tauri/src/commands/` is IPC-only (thin pass-throughs); React/TS frontend wraps `invoke()` in domain hooks. `MetadataStore` (`Arc<Mutex<Connection>>`) is the shared SQLite access point, already injected as managed state in Tauri.
- **Data Flow**: `ProfileContext` (app root) → `useProfile` hook (CRUD via `invoke()`) → `ProfileStore` (TOML at `~/.config/crosshook/profiles/`). Cover art: `useGameCoverArt` → `invoke('fetch_game_cover_art')` → `game_images::client` → `infer` MIME validation → `~/.local/share/crosshook/cache/images/{app_id}/` → `game_image_cache` SQLite row → absolute path returned → `convertFileSrc(path)` → `asset://` URL rendered in `<img>`.
- **Integration Points**: New `crosshook-core/src/steam_metadata/` and `crosshook-core/src/game_images/` modules; new `src-tauri/src/commands/game_metadata.rs` command file; `metadata/migrations.rs` v14 block; `tauri.conf.json` CSP + `capabilities/default.json` asset protocol scope; `ProfilesPage.tsx` card restructuring; `ProfileFormSections.tsx` preserved as composition point for `InstallPage` compatibility.

## Critical Files Reference

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/ProfilesPage.tsx`: Primary restructuring target — outer `CollapsibleSection("Advanced", defaultOpen=false)` wrapping everything must be removed; all 36k
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileFormSections.tsx`: 41k monolith shared with `InstallPage` via `reviewMode` prop — must stay backward-compatible until Phase 3 explicitly extracts sections
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: Holds local `rows` draft state — MUST use CSS `display: none` (not conditional unmount) on any tab switch
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: Canonical cache-first pattern to mirror exactly for `steam_metadata/client.rs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Sequential `if version < N` migration pattern; currently v13; v14 adds `game_image_cache`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts`: Canonical frontend hook — requestIdRef race guard, idle/loading/ready/stale/unavailable state machine, refresh() callback; mirror for `useGameMetadata`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs`: 13-line Tauri command template — exact model for new `game_metadata.rs` commands
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: Add new commands to `invoke_handler!`; `MetadataStore` already managed
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/tauri.conf.json`: Must add `img-src 'self' asset: http://asset.localhost` to CSP for `convertFileSrc` rendering
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/capabilities/default.json`: Must add `fs:allow-read-file` scoped to `$LOCALDATA/cache/images/**` and asset protocol scope
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css`: 90k; `crosshook-subtab-row/tab/tab--active` classes already exist (unused); new cover art + skeleton classes go here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css`: Sub-tab CSS variables already defined; cover art aspect-ratio + skeleton animation vars go here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/metadata/mod.rs`: `MetadataStore` — `with_conn()` for graceful degradation, `with_sqlite_conn()` for strict access; new `GameImageStore` methods delegate through this

## Patterns to Follow

- **Thin IPC Command**: Commands in `src-tauri/src/commands/` do zero business logic — clone `State<MetadataStore>`, delegate to `crosshook-core`, return `Result<T, String>`. See `protondb.rs` (13 lines) as the exact template.
- **Cache-First with Stale Fallback**: Check valid cache → live fetch on miss → persist on success → load expired cache on network failure → return `Unavailable` on total failure. See `protondb/client.rs:85-130`.
- **OnceLock HTTP Singleton**: `static HTTP_CLIENT: OnceLock<reqwest::Client>` per module with timeout + user_agent. One per new module.
- **Sequential Migration Guard**: `if version < 14 { migrate_13_to_14(conn)?; pragma_update("user_version", 14); }` — not `else if`. See `migrations.rs`.
- **Store Submodule Pattern**: `game_image_store.rs` takes `&Connection` directly; `MetadataStore` delegates via `with_sqlite_conn`. See `health_store.rs`.
- **Frontend Hook State Machine**: `requestIdRef = useRef(0)` race guard; `idle→loading→ready/stale/unavailable`; preserve previous snapshot during loading; expose `refresh()`. Mirror `useProtonDbLookup.ts` exactly.
- **CollapsibleSection Card**: `<CollapsibleSection title="..." className="crosshook-panel" meta={<Badge/>} defaultOpen>` is the card primitive. Use `meta` prop for collapsed header summaries.
- **Serde IPC Types**: All IPC-crossing types derive `Serialize + Deserialize`; enums `#[serde(rename_all = "snake_case")]`; optional fields `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- **CSS BEM + crosshook- prefix**: All new classes use `crosshook-` prefix. Variables in `variables.css`, classes in `theme.css`. Status badges use `crosshook-status-chip`.
- **`@radix-ui/react-tabs` already installed**: Phase 3 sub-tabs use existing CSS (`crosshook-subtab-*`) + Radix primitives. Zero new frontend deps for any phase.

## Cross-Cutting Concerns

- **`CustomEnvironmentVariablesSection` must never unmount during tab switches** (W1): CSS `display: none` required. This affects all Phase 3 tab panel implementation — if any sub-tab panel uses conditional rendering rather than CSS toggle, env var draft state is silently lost.
- **`injection.*` fields must not be surfaced** (W3): `GameProfile` has `injection.dll_paths` and `injection.inject_on_launch`. These are managed by the install pipeline, not user forms. Explicitly exclude from every new section component.
- **SVG rejection at Rust layer** (I1 — WARNING): `infer` crate magic-byte MIME validation before any write to disk. Allowlist: `image/jpeg`, `image/png`, `image/webp`. Never rely on Content-Type header alone. Code-ready `validate_image_bytes()` snippet in `research-security.md`.
- **Path traversal in image cache construction** (I2 — WARNING): `steam_app_id` validated as pure decimal integer; `canonicalize(base_dir)` + prefix assertion on joined path. Code-ready `safe_image_cache_path()` snippet in `research-security.md`.
- **`ProfileFormSections` backward compatibility**: `InstallPage` uses `reviewMode={true}`. Phase 1-2 changes must not touch `ProfileFormSections` structure. Phase 3 extracts sections but keeps `ProfileFormSections` as a thin composition wrapper.
- **`ProfileActions` must stay outside all panels**: Save/Delete/Duplicate/Rename bar must never be inside a tab panel or collapsible section.
- **`ProtonDB + EnvVars must stay co-located** (business rule): ProtonDB "Apply" writes directly to`custom_env_vars`. Separating them across tabs forces unnecessary tab-switching — they must remain in the same card/tab.
- **`MetadataStore` mutex is not a `RwLock`**: Never hold the lock across async awaits. Both SQL queries for cache-first pattern (valid check + stale fallback) each acquire/release separately.
- **`with_conn` silently degrades**: Returns `T::default()` when store is `disabled()`. Treat `None` cache results as normal code path, not errors.
- **Circular dependency must be fixed first** (Phase 0 prerequisite): `ui/ProtonPathField.tsx` imports `formatProtonInstallLabel` from `ProfileFormSections.tsx`. Extract to `utils/proton.ts` before any section splitting.
- **`AppSettingsData` must be updated on both Rust and TypeScript sides**: Round-tripped through IPC; missing `steamgriddb_api_key` in `types/settings.ts` will silently drop it on `settings_save`.
- **Asset protocol CSP scope must be narrow**: `$LOCALDATA/cache/images/**` only. A broad scope grants webview read access to all user files.

## Parallelization Opportunities

- **Phase 0 — full parallel**: UI cleanup tasks (dedup `FieldRow`→`InstallField`, consolidate `ProtonPathField`, extract `formatProtonInstallLabel`, replace `OptionalSection`) and backend infrastructure tasks (SQLite v14 migration, `GameImageStore`, `AppSettingsData` extension) touch entirely different files.
- **Phase 2 — partial parallel**: Rust `steam_metadata/` + `game_images/` modules and frontend `useGameMetadata`/`useGameCoverArt` hooks can develop in parallel against mock data (define types + IPC signatures first as contracts).
- **Phase 3 — high parallelism**: Extracting 6 section components from `ProfileFormSections` can be split across 3-4 agents (each agent takes 1-2 sections), provided the extraction order respects the circular dependency fix in Phase 0.
- **Security snippets are implementation-ready**: `validate_image_bytes()` and `safe_image_cache_path()` functions from `research-security.md` can be implemented directly without further design work.

## Implementation Constraints

- **`crosshook-core` owns all logic**: Tauri commands are IPC pass-throughs only. No business logic in `src-tauri/`.
- **Schema v14 is additive**: `CREATE TABLE IF NOT EXISTS` — safe, no rollback risk. `AppSettingsData` field uses `#[serde(default)]` — no settings migration needed.
- **`infer ~0.16` is the only new Rust dependency** (in `crosshook-core/Cargo.toml`). All other deps (`reqwest`, `rusqlite`, `chrono`, `uuid`, `serde`, `tracing`, `directories`) already present.
- **Zero new frontend dependencies** for any phase (`@radix-ui/react-tabs` already installed).
- **Phase 1 gates Phase 2**: Cover art CSS classes and card slot layout must be established in Phase 1 to avoid layout rework when art is wired in Phase 2.
- **`steam_app_id` is in TOML profile files**, not in the `profiles` SQLite table. `game_image_cache` joins by raw string, no FK to profiles.
- **Image rendering requires `convertFileSrc`** — raw filesystem paths as `<img src>` do not work in the Tauri webview. The CSP and capability scope must be configured before testing Phase 2 art display.
- **Cover art is enhancement-only**: Missing art must never block profile load, edit, save, or launch. Art slot hidden when unavailable — no broken image placeholders.
- **Keyboard + controller navigation must be preserved**: F2 rename, focus zones, gamepad D-pad. Test after any structural restructuring of `ProfilesPage`.

## Key Recommendations

- **Start Phase 0 as two parallel workstreams**: UI cleanup agent + backend infrastructure agent. They share no files.
- **Define IPC command signatures and TypeScript types before implementing hooks**: `SteamMetadataLookupResult` and `fetch_game_cover_art` return type serve as the contract between parallel Rust/TS development in Phase 2.
- **Copy `protondb/` module structure verbatim as a scaffold** for `steam_metadata/`: same file names (`mod.rs`, `client.rs`, `models.rs`), same state enum values, same cache key naming pattern (`namespace:entity:version:{id}`).
- **Use `health_store.rs` as the scaffold for `game_image_store.rs`**: same function signature pattern (`&Connection` params, `MetadataStore` public delegation methods).
- **CSS classes before component wiring**: Add `crosshook-profile-cover-art` + `crosshook-skeleton` to `theme.css` and cover art CSS variables to `variables.css` in Phase 1. This prevents layout thrash when Phase 2 adds the actual `<img>` element.
- **Phase 3 section extraction order**: Fix circular dep (`formatProtonInstallLabel`) → extract stateless sections first (Identity, Game, RunnerMethod, Trainer) → extract `RuntimeSection` last (most complex, runner-method conditional) → `CustomEnvironmentVariablesSection` is pre-extracted and must not be refactored.
- **Gate Phase 3 (sub-tabs) on Phase 1 feedback**: Sub-tab infrastructure is ready but shipping cards first reduces risk of a second restructuring if user feedback changes the navigation model.
- **SteamGridDB is Phase 3 only**: Steam Store API (no key friction) ships in Phase 2. `game_image_cache` table `source` column already handles SteamGridDB without a v15 migration.
