# Architecture Research: offline-trainers

## System Overview

CrossHook is a Tauri v2 desktop app with a Rust backend (`crosshook-core` library) and React/TypeScript frontend. All backend business logic lives in `crosshook-core`; the `src-tauri` shell exposes it via `#[tauri::command]` IPC handlers; the UI invokes these via `@tauri-apps/api/invoke`. Profiles are TOML files on disk; supplemental metadata (launch history, health, versions, community index, collections) is persisted in a single SQLite database at `~/.local/share/crosshook/metadata.db`.

## Relevant Components

### Core Library

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile` — the top-level profile struct containing `GameSection`, `TrainerSection`, `InjectionSection`, `SteamSection`, `RuntimeSection`, `LaunchSection`, `LocalOverrideSection`. **`TrainerSection` has `path`, `kind` (String), and `loading_mode` (enum `SourceDirectory`/`CopyToPrefix`)** — the primary extension point for a `TrainerType` enum.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` — CRUD for TOML profile files in `~/.config/crosshook/`. Handles load, save, rename, duplicate, delete, and preset management.
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: `LaunchRequest` — flat struct passed to the script runner at launch time. Holds `trainer_path`, `trainer_host_path`, `trainer_loading_mode`. The launch path reads `GameProfile` → builds `LaunchRequest` → dispatches to shell scripts.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: Assembles `std::process::Command` chains for `proton_run`, `steam_applaunch`, and `native` methods; uses `trainer_path`/`trainer_host_path` for trainer placement.
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` — singleton facade over SQLite, `Arc<Mutex<Connection>>`. All metadata operations go through this; `disabled()` constructor provides graceful degradation if DB unavailable.
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Sequential `PRAGMA user_version` migrations (currently v12). **The trainer hash infrastructure is already in schema v8/9: `version_snapshots` table stores `trainer_file_hash` (SHA-256 hex), `trainer_version`, `steam_build_id`.** New offline-trainer tables add a new migration step.
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `hash_trainer_file(path)` — reads file, returns `sha2::Sha256` hex digest. `compute_correlation_status()` — pure diff function comparing build_id + trainer hash. `upsert_version_snapshot()` / `lookup_latest_version_snapshot()` — persist and retrieve version records.
- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`: `ProfileHealthReport` / `HealthCheckSummary` — path-validity-based health scoring. Checks `trainer.path`, `game.executable_path`, Proton paths. **Integration point for offline readiness scoring** — add `HealthIssue` entries for trainer type warnings.
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `AppSettingsData` — global settings TOML (`~/.config/crosshook/settings.toml`); includes `community_taps` subscriptions. Could hold a `trainer_library_path` setting for the managed trainer store.
- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`: `CommunityTapStore` — git-clone/fetch pattern, commit-pinned sync. Parallel to how a trainer "library" store could manage versioned binary artifacts.
- `src/crosshook-native/crates/crosshook-core/src/install/models.rs`: `InstallGameRequest` / `InstallGameResult` — install workflow; `trainer_path` is mandatory input, validates file existence. Extension point for install-time trainer hashing and registration.
- `src/crosshook-native/crates/crosshook-core/src/install/service.rs`: Drives the Proton install subprocess with log attachment; builds `GameProfile` from `InstallGameRequest`.
- `src/crosshook-native/crates/crosshook-core/src/update/service.rs`: Game update (patch) workflow — runs installer over existing prefix.

### Tauri Command Layer

- `src/crosshook-native/src-tauri/src/commands/launch.rs`: IPC handlers for validate, preview, run, and version-check launch flows. Calls `hash_trainer_file` and `compute_correlation_status` from `crosshook_core::metadata` to detect trainer/game drift at launch time.
- `src/crosshook-native/src-tauri/src/commands/profile.rs`: Profile CRUD commands surfaced to the UI.
- `src/crosshook-native/src-tauri/src/commands/health.rs`: Triggers `HealthCheckSummary` and persists snapshots to `MetadataStore`.
- `src/crosshook-native/src-tauri/src/commands/install.rs`: Wraps `install::service` for the UI.
- `src/crosshook-native/src-tauri/src/lib.rs`: Tauri plugin registration, `MetadataStore` Tauri state setup.

### Frontend

- `src/crosshook-native/src/components/pages/LaunchPage.tsx`: Primary launch UI; drives version-correlation display.
- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`: Health dashboard — where offline readiness scores would surface.
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`: Profile management — trainer type badges/indicators would live here.
- `src/crosshook-native/src/types/launch.ts`: TypeScript type definitions mirroring `LaunchRequest` / `LaunchResult`.
- `src/crosshook-native/src/types/health.ts`: Health types.

## Data Flow

### Profile Lifecycle (relevant to offline-trainers)

```
User sets trainer_path (UI)
  → ProfilesPage calls invoke("save_profile")
  → src-tauri/commands/profile.rs
  → ProfileStore::save()
  → ~/.config/crosshook/<name>.toml
```

### Launch Flow (trainer path resolution)

```
LaunchPage invoke("launch_game") with LaunchRequest
  → launch.rs command handler
  → hash_trainer_file(request.trainer_path) — hash computed at launch time
  → upsert_version_snapshot() — stored in metadata.db
  → script_runner: build_proton_trainer_command() / build_trainer_command()
  → spawns shell command with trainer_path
```

### Health Check Flow

```
HealthDashboardPage invoke("run_health_check")
  → health.rs → ProfileStore::list_all() → profile::health::check_profile()
  → check_file_path("trainer.path", ...) — validates path exists
  → MetadataStore::upsert_health_snapshot()
  → returns ProfileHealthReport[]
```

### Version Snapshot Flow (trainer integrity)

```
version_store::hash_trainer_file(path) → SHA-256 hex
version_store::upsert_version_snapshot(profile_id, steam_app_id, steam_build_id, trainer_version, trainer_file_hash, ...)
version_store::compute_correlation_status(current_build_id, snapshot_build_id, current_hash, snapshot_hash, state_flags)
  → VersionCorrelationStatus::{Matched | GameUpdated | TrainerChanged | BothChanged | UpdateInProgress | Untracked}
```

## Integration Points

### 1. `TrainerSection` in `profile/models.rs`

The `kind: String` field in `TrainerSection` is the natural home for a `TrainerType` enum (`Standalone`, `AppBased`, `CheatEngine`, `OnlineOnly`). Adding this extends the profile TOML schema and requires a migration helper in `profile/legacy.rs` (pattern already exists for v1 migration).

### 2. New `trainer_library` Module in `crosshook-core`

Mirror the `community/` module pattern: a new `trainer_library/` module with:

- `models.rs` — `TrainerLibraryEntry`, `TrainerType`, `OfflineCapability`
- `store.rs` — file operations in `~/.local/share/crosshook/trainers/` (parallel to `community/taps/`)
- `hash_cache.rs` — stat-based invalidation layer wrapping `version_store::hash_trainer_file`

### 3. New SQLite Migration (v13) for Trainer Library Metadata

Add to `metadata/migrations.rs`:

```sql
CREATE TABLE trainer_library (
    trainer_id TEXT PRIMARY KEY,
    profile_id TEXT REFERENCES profiles(profile_id),
    trainer_type TEXT NOT NULL,
    offline_capability TEXT NOT NULL,
    managed_path TEXT,
    source_path TEXT,
    sha256_hash TEXT,
    hash_mtime INTEGER,  -- stat-based invalidation
    trainer_version TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
```

### 4. Health Check Extension (`profile/health.rs`)

Add `check_trainer_offline_readiness()` that emits `HealthIssue` entries with `severity: Warning` for `AppBased`/`OnlineOnly` trainer types. Hooks into `ProfileHealthReport.issues`.

### 5. New Tauri Commands

Add to `src-tauri/src/commands/`:

- `trainer_library.rs` — `register_trainer`, `list_library`, `verify_trainer_hash`, `check_offline_readiness`

### 6. `AppSettingsData` Extension

Add `trainer_library_path: Option<String>` to `settings/mod.rs::AppSettingsData` for user-configurable library location (defaults to `~/.local/share/crosshook/trainers/`).

## Key Dependencies

| Dependency       | Version          | Role                                                           |
| ---------------- | ---------------- | -------------------------------------------------------------- |
| `rusqlite`       | 0.39.0 (bundled) | SQLite metadata store — all persistent metadata                |
| `sha2`           | 0.11.0           | SHA-256 trainer file hashing — already used in `version_store` |
| `serde` + `toml` | 1.x / 1.1        | TOML profile serialization — all profiles and settings         |
| `chrono`         | 0.4              | Timestamps for snapshots and audit records                     |
| `directories`    | 6.0              | XDG-compliant path resolution for config/data dirs             |
| `tokio`          | 1.x              | Async runtime for Tauri commands and subprocess I/O            |
| `uuid`           | 1.x              | Entity IDs throughout the metadata store                       |

**No new crate dependencies are required for offline-trainer management.** The `sha2`, `rusqlite`, and `directories` crates already provide everything needed. The `keyring` crate is a future enhancement only (secure key storage for Aurora offline keys).

## Architecture Observations & Gotchas

- **`MetadataStore::disabled()` pattern**: The metadata store has a deliberate degraded mode (returns `T::default()` when unavailable). New trainer-library features must respect this — never panic or error fatally when `!store.is_available()`.
- **`TrainerSection.kind` is currently a freeform `String`**: No validation or enum exists yet. A `TrainerType` enum should be introduced carefully with `serde(rename_all = "snake_case")` and a `#[serde(default)]` fallback to `Standalone` to avoid breaking existing TOML profiles on upgrade.
- **`storage_profile()` / `effective_profile()` pattern**: Machine-specific paths (including `trainer.path`) are moved into `local_override` on save. Any trainer library managed path must go through `local_override.trainer.path` — the portable profile TOML should hold only a library reference ID, with the resolved path handled at runtime.
- **`hash_trainer_file` reads the entire file into memory**: For large trainer binaries this may be slow. The stat-based cache layer (mtime + size check before re-hashing) is essential.
- **`version_snapshots` has a rolling cap of `MAX_VERSION_SNAPSHOTS_PER_PROFILE` rows**: The trainer library model should not duplicate this; instead extend or join `version_snapshots` for integrity tracking.
- **Community taps use `git` subprocess (not a Rust git library)**: This avoids libgit2 linking complexity. Any future "tap"-style trainer distribution should follow the same subprocess pattern.
- **`LaunchRequest.trainer_host_path` vs `trainer_path`**: `trainer_host_path` is the host filesystem path (for Proton copy-to-prefix scenarios). The offline library store path resolution must produce the correct value for both fields.
