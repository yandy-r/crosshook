# Integration Research: install-game

## API Endpoints

### Existing Related Endpoints

- `list_proton_installs`: Returns detected Proton installs for a given Steam root hint. Implemented in `/src/crosshook-native/src-tauri/src/commands/steam.rs`.
- `default_steam_client_install_path`: Resolves a host Steam install path. Implemented in `/src/crosshook-native/src-tauri/src/commands/steam.rs`.
- `profile_list`: Lists saved profile names. Implemented in `/src/crosshook-native/src-tauri/src/commands/profile.rs`.
- `profile_load`: Loads a saved `GameProfile`. Implemented in `/src/crosshook-native/src-tauri/src/commands/profile.rs`.
- `profile_save`: Persists a `GameProfile` as TOML. Implemented in `/src/crosshook-native/src-tauri/src/commands/profile.rs`.
- `profile_delete`: Deletes a saved profile. Implemented in `/src/crosshook-native/src-tauri/src/commands/profile.rs`.
- `launch_game`: Starts a game process and log stream. Implemented in `/src/crosshook-native/src-tauri/src/commands/launch.rs`.
- `launch_trainer`: Starts a trainer process and log stream. Implemented in `/src/crosshook-native/src-tauri/src/commands/launch.rs`.
- `validate_launch`: Validates an existing `LaunchRequest`. Implemented in `/src/crosshook-native/src-tauri/src/commands/launch.rs`.
- `recent_files_load` / `recent_files_save`: Persist recent paths in local data storage. Implemented in `/src/crosshook-native/src-tauri/src/commands/settings.rs`.

### Route Organization

Tauri commands are not HTTP routes; they are registered centrally in `/src/crosshook-native/src-tauri/src/lib.rs` through `tauri::generate_handler![]`. Command files are grouped by domain in `/src/crosshook-native/src-tauri/src/commands/`, and each domain forwards into `crosshook-core` or a store. Install-game should follow that exact pattern with a new `/src/crosshook-native/src-tauri/src/commands/install.rs` and new entries in the existing `invoke_handler`.

## Database

### Relevant Tables

This feature does not use a database. Persistence is file-based:

- `~/.config/crosshook/profiles/*.toml`: Saved profiles via `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`.
- `~/.local/share/crosshook/recent.toml`: Recent paths via `/src/crosshook-native/crates/crosshook-core/src/settings/recent.rs`.
- `~/.config/crosshook/settings.toml`: App settings via `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`.

### Schema Details

- `GameProfile` in `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` is the persisted schema the install flow should reuse.
- Relevant profile fields for install-game:
  - `[game].name`
  - `[game].executable_path`
  - `[trainer].path`
  - `[runtime].prefix_path`
  - `[runtime].proton_path`
  - `[runtime].working_directory`
  - `[launch].method = "proton_run"`
- `ProfileStore::validate_name` in `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` is the existing name gate the install feature should reuse for generated profile names.

## External Services

- No hosted third-party API or database is involved.
- External runtime integrations are local filesystem/process integrations:
  - Proton install discovery in `/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`
  - Steam root detection in `/src/crosshook-native/src-tauri/src/commands/steam.rs`
  - Direct `proton run` process execution in `/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`
- `umu-run` is only a future compatibility reference. It is not part of the selected v1 integration surface.

## Internal Services

- `/src/crosshook-native/crates/crosshook-core/src/profile/`: Canonical profile schema and persistence.
- `/src/crosshook-native/crates/crosshook-core/src/launch/`: Existing process validation and process-building utilities that install-game should partially reuse.
- `/src/crosshook-native/crates/crosshook-core/src/steam/`: Proton discovery and Steam path probing.
- `/src/crosshook-native/crates/crosshook-core/src/settings/`: Existing local-data/config persistence patterns.
- `/src/crosshook-native/src/hooks/useProfile.ts`: Frontend profile synchronization and metadata updates after save/load.

## Configuration

- `HOME` and XDG paths matter because current stores resolve from `BaseDirs`:
  - profiles: `~/.config/crosshook/profiles`
  - recent files: `~/.local/share/crosshook/recent.toml`
- Install-game should default prefixes under `~/.local/share/crosshook/prefixes/<slug>`, which fits the existing data-vs-config split better than profiles do.
- Steam discovery checks:
  - `~/.local/share/Steam`
  - `~/.steam/root`
  - `~/.var/app/com.valvesoftware.Steam/data/Steam`
- Proton discovery also checks system roots such as `/usr/share/steam/compatibilitytools.d`.
- Long-running process logs currently write to `/tmp/crosshook-logs`, so install-game can either reuse that location or factor log-path creation into a shared helper.
