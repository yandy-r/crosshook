# Integration Research: ProtonUp Integration

## Overview

CrossHook has a complete integration infrastructure already in place for ProtonUp. `libprotonup = "0.11.0"` is pinned directly in `crosshook-core/Cargo.toml`. The SQLite metadata DB at schema v18 provides the `external_cache_entries` table for TTL-based GitHub API caching and `with_sqlite_conn` / `put_cache_entry` / `get_cache_entry` methods directly on `MetadataStore`. The existing `steam` module already implements Proton discovery (`discover_compat_tools`, `list_proton_installs`) whose data model aligns closely with what libprotonup produces. Progress streaming in the codebase currently uses `AppHandle::emit` + event subscriptions rather than Tauri Channels, but Channels are the recommended v2 pattern for ordered download progress.

---

## Relevant Files

**Tauri command layer (src-tauri)**

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — app setup, managed state registration, full invoke_handler list
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs` — command module declarations
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs` — `list_proton_installs`, `default_steam_client_install_path`, `auto_populate_steam`; reference for Steam path resolution pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/migration.rs` — reference for Proton migration commands and path sanitization pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/update.rs` — reference for long-running process with event-based progress streaming
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/prefix_deps.rs` — reference for `AppHandle::emit` + managed state lock pattern

**crosshook-core crate**

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml` — contains `libprotonup = "0.11.0"` dependency
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/mod.rs` — `ProtonInstall`, `discover_compat_tools`, `discover_steam_root_candidates`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` — filesystem-based Proton discovery logic; important for dedup with libprotonup
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/models.rs` — `ProtonInstall`, `SteamAutoPopulateFieldState` types
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore`, `get_cache_entry`, `put_cache_entry`, `with_sqlite_conn`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — cache read/write/evict primitives; `MAX_CACHE_PAYLOAD_BYTES` enforced here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — schema v18, all 18 tables; no protonup-specific table yet
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/db.rs` — SQLite connection open/configure; WAL mode, FK enforcement, `application_id = 0x43484B00`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — reference for external HTTP client: `OnceLock<reqwest::Client>`, TTL cache via `external_cache_entries`, stale fallback
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `AppSettingsData`; `steam_client_install_path` is NOT a settings field (resolved at runtime per command call); `default_proton_path` is a settings field

---

## API Endpoints (Tauri Commands)

### Existing Commands — Relevant to ProtonUp

| Command                             | File                       | Notes                                                                                                                                                        |
| ----------------------------------- | -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `list_proton_installs`              | `commands/steam.rs:34`     | Returns `Vec<ProtonInstall>` from local filesystem via `discover_compat_tools`. This is the **existing** installed-versions source; do not replace, augment. |
| `default_steam_client_install_path` | `commands/steam.rs:8`      | Resolves Steam root from env var or home dir candidates. Used by the new protonup commands to derive the install path.                                       |
| `check_proton_migrations`           | `commands/migration.rs:25` | Scans profiles for stale Proton paths; uses same Steam discovery infrastructure.                                                                             |
| `apply_proton_migration`            | `commands/migration.rs:63` | Atomic Proton path update per profile; shows metadata sync + health snapshot invalidation pattern.                                                           |

### New Commands Required

The following commands must be registered in `src-tauri/src/lib.rs` `invoke_handler![]`:

```
protonup_list_available_versions  — list GitHub releases for a given CompatTool (with TTL cache)
protonup_list_installed_versions  — filesystem listing via libprotonup::AppInstallations
protonup_install_version          — streaming download + verify + extract (Tauri Channel)
protonup_cancel_install           — cancel in-flight download
protonup_delete_version           — remove a version directory from compatibilitytools.d
```

### IPC Patterns

All existing Tauri commands follow a consistent pattern:

- `snake_case` command names matching `#[tauri::command]` fn names
- `Result<T, String>` return type — errors are `.to_string()` converted at the command boundary
- Async commands use `tauri::async_runtime::spawn_blocking` for CPU-bound sync work (e.g., `auto_populate_steam`)
- Long-running operations use `AppHandle::emit` with named event strings (e.g., `"update-log"`, `"prefix-dep-log"`, `"prefix-dep-complete"`) — **not yet Tauri v2 Channels**
- Managed state registered via `.manage(...)` in `lib.rs` setup and accessed via `State<'_, T>` parameters
- New install state (for cancellation) follows `UpdateProcessState` / `PrefixDepsInstallState` pattern: `Mutex<Option<T>>` wrapped in a `pub struct`, `impl new()`, registered via `.manage()`

**Note on Channels vs Events**: The existing codebase uses `AppHandle::emit` for streaming (update-log, prefix-dep-log). The `research-external.md` recommends `Channel<T>` for the download progress stream (Tauri v2 best practice for ordered, typed streams). Either works; Channel is preferred for new download progress to avoid event name collisions and enable typed payloads. The `prefix-dep-log` pattern (event name + payload struct) remains viable if Channel introduces friction.

---

## Database Schema

### Current Schema: v18 (18 tables)

The schema is built incrementally via `metadata/migrations.rs`. Current migration goes to v18.

#### Tables Directly Relevant to ProtonUp

**`external_cache_entries`** (created in migration 3→4)

```sql
CREATE TABLE external_cache_entries (
    cache_id        TEXT PRIMARY KEY,
    source_url      TEXT NOT NULL,
    cache_key       TEXT NOT NULL UNIQUE,
    payload_json    TEXT,                    -- NULL when payload > MAX_CACHE_PAYLOAD_BYTES
    payload_size    INTEGER NOT NULL DEFAULT 0,
    fetched_at      TEXT NOT NULL,
    expires_at      TEXT,                    -- NULL = no expiry; compared as TEXT ISO-8601
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
```

**Usage for ProtonUp**: cache GitHub release listings with a 6-hour TTL.

- Cache key pattern (mirrors ProtonDB): `"github:proton-releases:GEProton"` / `"github:proton-releases:WineGE"`
- `MAX_CACHE_PAYLOAD_BYTES` is defined in `metadata/models.rs` — must be verified before storing full release JSON (30 releases ≈ 50-100 KB; should be within limit)
- Access via `MetadataStore::get_cache_entry` / `put_cache_entry` (wraps `cache_store` functions, holds `Arc<Mutex<Connection>>` lock)

**`profiles`** (created in migration 0→1)

```sql
CREATE TABLE profiles (
    profile_id TEXT PRIMARY KEY,
    current_filename TEXT NOT NULL UNIQUE,
    current_path TEXT NOT NULL,
    game_name TEXT,
    launch_method TEXT,
    content_hash TEXT,
    is_favorite INTEGER NOT NULL DEFAULT 0,
    ...
);
```

Relevant because profiles reference Proton paths. After installing a new version, the migration commands (`check_proton_migrations`, `apply_proton_migration`) can update stale paths.

#### Tables NOT Needed (but awareness required)

- `version_snapshots` — tracks trainer/steam build versions per profile; no protonup relevance
- `trainer_hash_cache` / `offline_readiness_snapshots` — trainer-specific; no protonup relevance
- `prefix_dependency_state` — winetricks/protontricks package state; no protonup relevance

#### New Table? — `installed_proton_versions`

The feature spec may call for persisting installed version metadata (name, path, size, install date). The current codebase has no such table. Options:

1. **No new table** — derive from filesystem at runtime using `libprotonup::AppInstallations::list_installed_versions()` (recommended as the runtime source of truth per `research-external.md`)
2. **New table** (schema v19) — would allow install history, quick startup cache, and deletion audit. Only justified if the UX requires displaying install date/size without filesystem I/O on every load.

If a new table is added, add it as `migrate_18_to_19` in `metadata/migrations.rs` following the established pattern, bump schema version to 19.

---

## External Services

### 1. libprotonup (primary, already in Cargo.toml)

**Crate**: `libprotonup = "0.11.0"` in `crosshook-core/Cargo.toml:28`
**Purpose**: GitHub Releases API client, streaming download, SHA-512 verification, tar extraction

Key public API (see `research-external.md` for full details):

- `libprotonup::downloads::list_releases(&compat_tool)` → `Vec<Release>` (30 releases, no pagination)
- `libprotonup::downloads::download_to_async_write(url, writer)` → streaming to any `AsyncWrite`
- `libprotonup::downloads::download_file_into_memory(url)` → fetch `.sha512sum` text
- `libprotonup::apps::App::Steam.detect_installation_method()` → `AppInstallations` (Native or Flatpak)
- `libprotonup::apps::AppInstallations::list_installed_versions()` → `Vec<String>` directory names
- `libprotonup::apps::AppInstallations::default_install_dir()` → `~/.steam/steam/compatibilitytools.d/`
- `libprotonup::sources::CompatTool::sources_for_app(&App::Steam)` → tools available for Steam
- `libprotonup::files::unpack_file(compat_tool, download, reader, install_path)` → extract archive

**CompatTool name strings** (case-insensitive `FromStr`):

- `"GEProton"` — GE-Proton for Steam
- `"WineGE"` — Wine-GE for Lutris (installs to `~/.local/share/lutris/runners/wine/`, not `compatibilitytools.d/`)

**Limitation**: `list_releases` returns only first 30 (no pagination). `Release` struct does NOT include `published_at` — dates cannot be shown from `list_releases` alone without a separate GitHub API call or direct HTTP fetch.

### 2. GitHub Releases REST API (via libprotonup)

libprotonup calls:

- `https://api.github.com/repos/GloriousEggroll/proton-ge-custom/releases`
- `https://api.github.com/repos/GloriousEggroll/wine-ge-custom/releases`

**Rate limits**: 60 req/hr unauthenticated. Cache is mandatory. After a `reqwest::Error` with `status() == Some(StatusCode::FORBIDDEN)`, serve stale cache.

**Required headers** (handled by libprotonup's reqwest client): `User-Agent`, `Accept`. Do not replicate a second HTTP client unless bypassing libprotonup for pagination.

### 3. reqwest (HTTP client, already in Cargo.toml)

**Version**: `0.13.2` with `rustls` (no OpenSSL), `json` features
**Usage**: ProtonDB client (`protondb/client.rs`) uses a `OnceLock<reqwest::Client>` singleton initialized lazily. The same pattern should be used for any libprotonup-bypass HTTP calls (e.g., if fetching GitHub API directly for `published_at`).

**Client initialization pattern** (from `protondb/client.rs:175`):

```rust
static PROTONDB_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn protondb_http_client() -> Result<&'static reqwest::Client, ProtonDbError> {
    // lazy init with timeout and user-agent
}
```

### 4. Steam Filesystem Integration (local, no network)

**Handled by existing `crosshook-core::steam` module**:

- `discover_steam_root_candidates(path, diagnostics)` — finds `~/.steam/root`, `~/.local/share/Steam`, Flatpak variant
- `discover_compat_tools(candidates, diagnostics)` — scans `compatibilitytools.d/` + system paths
- Returns `Vec<ProtonInstall>` with `name`, `path`, `aliases`, `is_official`

**libprotonup overlap**: Both CrossHook's existing steam module and libprotonup scan `compatibilitytools.d/`. The existing `list_proton_installs` Tauri command should continue to be the source for "what Proton version to use in profiles". libprotonup's `list_installed_versions()` returns simpler `Vec<String>` (just directory names) and is the right source for the ProtonUp management UI. The two should remain separate rather than trying to unify them.

---

## Internal Services

### Module Communication

```
src-tauri (Tauri commands)
  └─ crosshook_core::steam          — Proton install discovery
  └─ crosshook_core::metadata       — SQLite store (cache, profile sync)
  └─ crosshook_core::settings       — AppSettingsData (steam_client_install_path NOT stored here)
  └─ libprotonup                    — via crosshook_core (transitive)
```

**Important**: libprotonup is a dependency of `crosshook-core`, not of `src-tauri` directly. New ProtonUp service code must live in `crosshook-core` with thin Tauri command wrappers in `src-tauri`. This matches the architecture requirement: "business logic lives in `crosshook-core`; keep `src-tauri` thin."

### MetadataStore Access Pattern

`MetadataStore` wraps `Arc<Mutex<Connection>>`. All access goes through `with_conn` / `with_conn_mut` / `with_sqlite_conn`. Long-running async operations (download) must NOT hold the Mutex while awaiting — acquire lock only for reads/writes, not during HTTP/IO.

Pattern (from `protondb/client.rs`):

```rust
// Read from cache BEFORE starting async work
if let Some(cached) = metadata_store.get_cache_entry(&cache_key)? { ... }

// Do async network/IO work WITHOUT the mutex
let result = fetch_from_github().await?;

// Write to cache AFTER async work completes
metadata_store.put_cache_entry(source_url, &cache_key, &payload, Some(&expires_at))?;
```

### SettingsStore Relevance

`AppSettingsData` (in `settings/mod.rs:133`) does NOT have a `steam_client_install_path` field. The steam path is resolved per-command call by `default_steam_client_install_path()` or passed as a parameter. The ProtonUp install path should use:

1. If user has configured a custom Steam path (passed via parameter from frontend) → `AppInstallations::Custom(path)`
2. Otherwise → `App::Steam.detect_installation_method().await` (libprotonup autodetect)

If a user-configurable default install path for ProtonUp versions is needed, it could be added to `AppSettingsData` as `protonup_install_path: String` (empty = auto-detect). This would be a new settings field, persisted in `~/.config/crosshook/settings.toml`.

### Cancellation Pattern

`UpdateProcessState` and `PrefixDepsInstallState` both use `Mutex<Option<...>>` stored as managed Tauri state:

```rust
pub struct ProtonUpInstallState {
    cancellation: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}
```

Register via `.manage(ProtonUpInstallState::new())` in `lib.rs`. The install command takes `State<'_, ProtonUpInstallState>` and stores a cancellation sender. `protonup_cancel_install` acquires the mutex, takes the sender, and calls `.send(())`.

---

## Configuration

### Environment Variables

- `STEAM_COMPAT_CLIENT_INSTALL_PATH` — read by `default_steam_client_install_path()` (`commands/steam.rs:10`); libprotonup's `dirs` crate also reads this via `~/.steam` paths
- `HOME` — fallback for Steam path discovery
- `WEBKIT_DISABLE_DMABUF_RENDERER` — set at startup in `lib.rs`; not relevant to ProtonUp

### Settings Files

- `~/.config/crosshook/settings.toml` — `AppSettingsData`; no protonup-specific fields yet
- `~/.local/share/crosshook/metadata.db` — SQLite; path from `BaseDirs::data_local_dir().join("crosshook/metadata.db")`

### Steam Paths Resolved at Runtime

- Native Steam: `~/.local/share/Steam` or `~/.steam/root`
- Flatpak Steam: `~/.var/app/com.valvesoftware.Steam/data/Steam`
- Install target: `<steam_root>/compatibilitytools.d/<version_name>/`
- System tools (read-only): `/usr/share/steam/compatibilitytools.d/`, `/usr/local/share/steam/compatibilitytools.d/`

---

## Gotchas and Edge Cases

- **libprotonup is in crosshook-core, not src-tauri**: `src-tauri/Cargo.toml` has no `libprotonup` direct dep. New service code using libprotonup types must live in `crosshook-core`. The Tauri command wrapper in `src-tauri` imports from `crosshook_core::protonup` (future module).

- **Mutex must not be held during async IO**: `MetadataStore` uses `Arc<Mutex<Connection>>`. Do not hold the lock while awaiting download operations. Read cache → release lock → do network work → re-acquire lock to write cache.

- **Steam path is NOT in AppSettingsData**: There is no `steam_client_install_path` field in `AppSettingsData`. The path comes from `default_steam_client_install_path()` (env var → filesystem candidates). ProtonUp commands must either accept it as a parameter or call that function internally.

- **ProtonInstall overlap**: The existing `discover_compat_tools` in `steam/proton.rs` and libprotonup's `list_installed_versions()` both scan `compatibilitytools.d/`. They serve different UI purposes and should not be merged. `discover_compat_tools` returns rich `ProtonInstall` with aliases for profile path resolution; libprotonup returns `Vec<String>` for the ProtonUp management UI.

- **`Release` struct missing `published_at`**: libprotonup's `Release` struct does not deserialize `published_at` from the GitHub API response. Showing release dates requires either a direct GitHub API call or accepting that dates are unavailable.

- **Cache payload size limit**: `MAX_CACHE_PAYLOAD_BYTES` (in `metadata/models.rs`) is enforced by `cache_store::put_cache_entry`. If the 30-release JSON exceeds this, the payload is stored as NULL. Store a stripped payload (tag names + sizes only) as a fallback.

- **WineGE installs to Lutris paths**: `AppInstallations` for WineGE targets `~/.local/share/lutris/runners/wine/` — NOT `compatibilitytools.d/`. If CrossHook only supports Steam/Proton game launching, WineGE should be excluded from the install UI or clearly labeled as Lutris-only.

- **No external protonup binary**: The integration uses `libprotonup` (Rust crate) directly. There is no `protonup-qt` or `protonup` binary dependency. The "binary not found" failure mode does not apply.

- **Migration command integration**: After a new Proton version is installed, the existing `check_proton_migrations` / `apply_proton_migration` commands can be called from the ProtonUp UI to update stale profile paths. No new migration logic is needed.

- **Tauri Channel not used yet**: The codebase currently uses `AppHandle::emit` + event strings for all streaming. The `install_prefix_dependency` command is the closest analog — it emits `prefix-dep-log` and `prefix-dep-complete` events. Tauri v2 `Channel<T>` is technically superior for download progress but requires frontend changes to subscribe. Either pattern works; decide before implementation.

---

## Other Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/research-external.md` — full libprotonup API surface, GitHub rate limits, integration patterns, code examples
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/research-technical.md` — technical architecture decisions
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protonup-integration/feature-spec.md` — feature specification
- [libprotonup docs.rs](https://docs.rs/libprotonup/latest/libprotonup/)
- [Tauri v2 Channels](https://v2.tauri.app/develop/calling-frontend/)
- [Tauri v2 State management](https://v2.tauri.app/develop/state-management/)
