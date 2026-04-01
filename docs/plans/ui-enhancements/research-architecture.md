# Architecture Research: ui-enhancements

## System Overview

CrossHook is a Tauri v2 native Linux desktop app (AppImage target) with a React 18 + TypeScript frontend communicating with a Rust backend via `invoke()` IPC. Business logic is centralized in `crosshook-core` (a Rust crate), while the Tauri layer (`src-tauri/`) is a thin shell that registers `#[tauri::command]` handlers and manages app-level state via `.manage()`. The Profiles page is the primary modification target: it currently hides all editing fields behind a single `CollapsibleSection("Advanced", defaultOpen=false)`.

## Relevant Components

### Frontend (React/TypeScript)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/ProfilesPage.tsx`: Main page; 36k, owns all profile editor state and orchestration. The entire form is wrapped in a single `CollapsibleSection` with `defaultOpen={false}`. This is the primary restructuring target.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileFormSections.tsx`: 41k monolith holding all profile form fields (identity, game path, runner method, steam/proton/native sections, env vars, trainer, ProtonDB lookup). Used by both `ProfilesPage` and `InstallPage` (via `reviewMode` prop).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProtonDbLookupCard.tsx`: 15k standalone card wrapping `useProtonDbLookup` — the template to replicate for Steam metadata lookup.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: 7.8k; holds local `rows` state that must not be unmounted during tab switches (requires CSS `display: none`, not conditional rendering).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileActions.tsx`: 6.9k; Save/Delete/Duplicate/Rename actions bar — must remain outside any collapsible.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ui/CollapsibleSection.tsx`: `<details>`-based collapsible; supports `defaultOpen`, controlled `open`/`onToggle`, and `meta` slot for inline badges.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ui/ThemedSelect.tsx`: Radix Select wrapper used for all dropdowns including the profile selector.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/HealthBadge.tsx`: 4.1k badge for profile health status — already rendered inline in the `CollapsibleSection` meta slot.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/OfflineStatusBadge.tsx`: Similar badge for offline readiness.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/InstallPage.tsx`: 17k page that renders `ProfileFormSections` in `reviewMode`. Structural changes to `ProfileFormSections` must preserve this contract.

### State and Context

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/context/ProfileContext.tsx`: Wraps `useProfile` hook; provides `profile`, `updateProfile`, `dirty`, `saveProfile`, `selectProfile`, plus derived values (`launchMethod`, `steamClientInstallPath`, `targetHomePath`). This is the single source of truth for active profile state across all pages.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/context/ProfileHealthContext.tsx`: Provides health check state (`healthByName`, `cachedSnapshots`, `trendByName`, `batchValidate`, `revalidateSingle`).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts`: 46k hook; owns all profile CRUD against the Tauri backend via `invoke()`. The `updateProfile(updater)` function accepts an immutable updater `(current: GameProfile) => GameProfile`.

### Hooks (patterns to follow)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts`: The canonical pattern for external API hooks — manages `idle/loading/ready/stale/unavailable` state, handles race conditions via `requestIdRef`, supports `refresh()` for force-refresh, and integrates directly with the backend command via `invoke('protondb_lookup', ...)`. The new `useGameMetadata` and `useGameCoverArt` hooks should mirror this exactly.

### Backend (Rust — crosshook-core)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: Reference implementation for external API lookup with MetadataStore caching. Pattern: normalize input → check valid cache → fetch live → persist → return stale on failure.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` struct (SQLite via `Arc<Mutex<Connection>>`); lives at `~/.local/share/crosshook/metadata.db`. Key methods: `put_cache_entry`, `with_sqlite_conn`. Can be `disabled()` for graceful degradation.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `get_cache_entry`/`put_cache_entry`/`evict_expired_cache_entries` against `external_cache_entries` table. Enforces `MAX_CACHE_PAYLOAD_BYTES` (512 KiB) — images exceed this, hence filesystem caching.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Sequential `if version < N { migrate_N_to_N+1 }` pattern with `PRAGMA user_version`. Currently at schema v13. New `game_image_cache` table requires a v14 migration following this exact pattern.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: Types (`ProtonDbLookupResult`, `ProtonDbLookupState`, `ProtonDbSnapshot`, etc.) — directly parallel to what `steam_metadata/models.rs` must define.

### Tauri Layer (src-tauri)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: Entry point; initializes stores via `.manage()`, registers all `#[tauri::command]` handlers in `invoke_handler!`. New Steam commands must be added here.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs`: Minimal 13-line command file — the exact template for the new `steam.rs` command module (reuse pattern, not expand the existing `commands/steam.rs` which handles `auto_populate_steam` and `list_proton_installs`).
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs`: Module registry — new `steam_metadata` or expanded `steam` module must be declared here.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/capabilities/default.json`: Currently has no `assetProtocol` permission. Must add `fs:allow-read` scope for `$LOCALDATA/cache/images/**` to enable `convertFileSrc`.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/tauri.conf.json`: CSP is currently `default-src 'self'; script-src 'self'` — missing `img-src asset: http://asset.localhost` required for local image rendering via Tauri asset protocol.

### Styles

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css`: CSS custom properties including `--crosshook-subtab-min-height: 40px` and `--crosshook-subtab-padding-inline: 16px` — sub-tab infrastructure already defined.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css`: 90k stylesheet; contains `.crosshook-subtab-row`, `.crosshook-subtab`, `.crosshook-subtab--active` classes already present and unused. New cover art and skeleton classes go here.

## Data Flow

### Profile Display Flow (current)

```
ProfileProvider (App root)
  └── useProfile hook → invoke('profile_load') / invoke('profile_save')
        → ProfileStore (TOML files at ~/.config/crosshook/profiles/)
  └── ProfileContext → ProfilesPage
        → ProfileFormSections (all fields in one collapsed section)
        → ProfileActions
```

### ProtonDB Lookup Flow (template for Steam metadata)

```
ProfileFormSections
  └── ProtonDbLookupCard
        └── useProtonDbLookup(appId)
              → invoke('protondb_lookup', { appId, forceRefresh })
                    → protondb::client::lookup_protondb()
                          → MetadataStore::get_cache_entry (external_cache_entries table)
                          → reqwest GET protondb.com/api (on cache miss)
                          → MetadataStore::put_cache_entry (upsert, TTL = 6h)
```

### New Steam Metadata + Cover Art Flow (proposed)

```
ProfileFormSections (or extracted GameSection)
  └── useGameMetadata(steamAppId)
        → invoke('fetch_game_metadata', { appId })
              → steam_metadata::client::fetch_steam_metadata()
                    → MetadataStore::get_cache_entry (key: steam:appdetails:v1:{id})
                    → reqwest GET store.steampowered.com/api/appdetails (on miss)
                    → MetadataStore::put_cache_entry (TTL = 24h)
  └── useGameCoverArt(steamAppId)
        → invoke('fetch_game_cover_art', { appId, imageType })
              → game_images::client::download_and_cache()
                    → GameImageStore::get (game_image_cache table)
                    → reqwest GET Steam CDN (on miss)
                    → infer::get MIME validation (reject SVG/non-image)
                    → fs::write ~/.local/share/crosshook/cache/images/{id}/cover_steam_cdn.jpg
                    → GameImageStore::put (upsert)
              → returns Option<String> (absolute filesystem path)
  └── <img src={convertFileSrc(localPath)} /> via Tauri asset protocol
```

### IPC Invoke Pattern (frontend → backend)

All IPC calls use `invoke()` from `@tauri-apps/api/core` with camelCase argument names on the frontend mapping to `snake_case` Rust parameter names (Tauri handles this translation). Results are typed generics: `invoke<ReturnType>('command_name', { argName: value })`.

## Integration Points

### Where New Code Plugs In

1. **`crosshook-core/src/`** — Add two new modules at the core level:
   - `steam_metadata/` (mod.rs, client.rs, models.rs) — mirrors `protondb/` structure exactly
   - `game_images/` (mod.rs, client.rs, cache.rs, models.rs) — image download + validation + filesystem caching
   - `metadata/game_image_store.rs` — SQLite CRUD for `game_image_cache` table (add to `metadata/mod.rs`)
   - `metadata/migrations.rs` — add `if version < 14 { migrate_13_to_14(conn)?; }` block

2. **`src-tauri/src/commands/`** — New `steam_metadata.rs` command file (or extend `steam.rs`) with `fetch_game_metadata` and `fetch_game_cover_art` commands. Register both in `lib.rs` `invoke_handler!`.

3. **`src-tauri/capabilities/default.json`** — Add `fs:allow-read` with scope `$LOCALDATA/cache/images/**` and `assetProtocol` permission.

4. **`src-tauri/tauri.conf.json`** — Extend `csp` to include `img-src 'self' asset: http://asset.localhost`.

5. **`ProfilesPage.tsx`** — Remove the outer `CollapsibleSection("Advanced")` wrapper; promote each logical group to its own `CollapsibleSection` + `crosshook-panel`. Add cover art display conditional on `steam_app_id` being set and art being cached.

6. **`ProfileFormSections.tsx`** — Remains as composition point for `InstallPage` compatibility; section extraction in Phase 3 should reduce it to a thin wrapper calling extracted section components.

7. **`styles/theme.css`** — Add `crosshook-profile-cover-art` (aspect-ratio: 460/215, object-fit: cover) and `crosshook-skeleton` (shimmer keyframe animation) CSS classes.

### Components Directly Affected by Restructuring

- `ProfilesPage.tsx` — major restructuring (Phase 0–2)
- `ProfileFormSections.tsx` — section extraction (Phase 3); `reviewMode` contract must be preserved
- `CustomEnvironmentVariablesSection.tsx` — must remain mounted (CSS show/hide, not conditional render)
- `ProtonDbLookupCard.tsx` — stays co-located with env vars (business rule: ProtonDB writes to env vars)

### Components That Must Not Change (risk of regression)

- `InstallPage.tsx` — consumes `ProfileFormSections` with `reviewMode={true}`; structural changes to `ProfileFormSections` must be backward-compatible until Phase 3 explicitly refactors it
- `OnboardingWizard.tsx` — also consumes profile form patterns; test after restructuring

## Key Dependencies

### External Libraries (Frontend)

| Package                  | Version | Purpose                                                                    |
| ------------------------ | ------- | -------------------------------------------------------------------------- |
| `@radix-ui/react-tabs`   | ^1.1.13 | Sub-tab primitives (Phase 3) — already installed, CSS already defined      |
| `@radix-ui/react-select` | ^2.2.6  | `ThemedSelect` dropdown wrapper — used in profile selector and form fields |
| `@tauri-apps/api`        | ^2.0.0  | `invoke()`, `listen()`, `convertFileSrc()`                                 |
| `@tauri-apps/plugin-fs`  | ^2.0.0  | File system access (already plugged in via `tauri_plugin_fs::init()`)      |

### External Libraries (Rust — new)

| Crate      | Purpose                                                        | Status                                                   |
| ---------- | -------------------------------------------------------------- | -------------------------------------------------------- |
| `reqwest`  | HTTP client for Steam Store API + image download               | Already in Cargo.toml (used by ProtonDB)                 |
| `rusqlite` | SQLite driver for `game_image_cache`                           | Already in Cargo.toml                                    |
| `infer`    | Magic-byte MIME detection for image validation (SVG rejection) | New dependency — must add to `crosshook-core/Cargo.toml` |

### Internal Modules Consumed

- `MetadataStore` — shared across ProtonDB, new Steam metadata, and game image cache; passed as Tauri managed state (`State<'_, MetadataStore>`)
- `ProfileStore` — profile CRUD at `~/.config/crosshook/profiles/`
- `SettingsStore` / `AppSettingsData` — must add `steamgriddb_api_key: Option<String>` with `#[serde(default)]` for Phase 3
