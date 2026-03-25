# Integration Research: proton-optimizations

## API Endpoints

### Existing Related Endpoints

- `profile_list`: Lists saved TOML profile names through `ProfileStore::list` so the UI can populate the profile selector.
- `profile_load`: Loads a single `GameProfile` from TOML so React can hydrate editor state before building a launch request.
- `profile_save`: Persists the full `GameProfile` document through `ProfileStore::save`; today this is the only write path for profile data.
- `profile_delete`: Deletes a profile and performs best-effort launcher cleanup for non-native profiles.
- `profile_rename`: Renames an existing TOML profile file.
- `validate_launch`: Runs Rust-side validation for the current `LaunchRequest` before launch.
- `launch_game`: Spawns the game phase using the resolved launch method and streams launcher logs back to the app.
- `launch_trainer`: Spawns the trainer phase using the resolved launch method and streams launcher logs back to the app.
- `default_steam_client_install_path`: Supplies a default Steam client root used when React builds launch context for Proton and Steam flows.
- `list_proton_installs`: Enumerates detected Proton installs for editor dropdowns and runtime selection.
- `settings_load` / `settings_save`: Maintain settings used alongside profile interactions, including last-used profile tracking.
- `recent_files_load` / `recent_files_save`: Maintain recent path metadata that currently updates on explicit profile save/load flows.

### Route Organization

CrossHook does not expose HTTP routes. Integration points are Tauri commands registered in `src/crosshook-native/src-tauri/src/lib.rs` and invoked from React with `invoke()` from `@tauri-apps/api/core`. The frontend builds a typed `LaunchRequest` in `src/crosshook-native/src/App.tsx`, passes full `GameProfile` documents through `useProfile.ts`, and sends them to thin Tauri command handlers under `src-tauri/src/commands/`. Those command handlers defer most business logic to `crosshook-core`, especially `profile/` for TOML persistence and `launch/` for validation and process construction.

## Database

### Relevant Tables

- none: This feature uses TOML profile persistence, not a database.

### Schema Details

Profiles are stored as TOML files under `~/.config/crosshook/profiles` via `ProfileStore` in `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`. The current persisted schema is the Rust `GameProfile` defined in `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`, mirrored by the TypeScript `GameProfile` in `src/crosshook-native/src/types/profile.ts`. Relevant sections for this feature are:

- `game`: name and executable path
- `trainer`: trainer path and trainer type
- `steam`: Steam app ID, compatdata path, Proton path, launcher metadata
- `runtime`: prefix path, Proton path, working directory
- `launch`: currently only `method`

The planned integration point is a new `launch.optimizations.enabled_option_ids` subsection persisted in both Rust and TypeScript. Existing save behavior writes the whole profile document at once via `profile_save`; there is no section-specific persistence path yet, which is why the spec recommends adding a dedicated optimization-save command instead of reusing the full save-refresh-reload flow.

## External Services

CrossHook does not depend on SaaS APIs for this feature. The meaningful external integrations are host tools and runtime conventions:

- Proton itself, launched directly through the configured `runtime.proton_path` for `proton_run`
- MangoHud, which is a host wrapper binary rather than a library or API
- GameMode, exposed through the `gamemoderun` host wrapper
- optional CachyOS `game-performance` wrapper, which is distro-specific
- the user’s desktop session environment, including `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, and `DBUS_SESSION_BUS_ADDRESS`

These are integration boundaries rather than network services. For `proton_run`, CrossHook can control them directly by constructing the process command in Rust. For `steam_applaunch`, Steam helper scripts are a separate boundary and not part of the required scope for this feature.

## Internal Services

- `src/crosshook-native/src/App.tsx`: Builds the current `launchRequest` from the selected `GameProfile`, launch method, and derived Steam client install path.
- `src/crosshook-native/src/hooks/useProfile.ts`: Owns profile editor state, normalization, explicit save behavior, and current `invoke('profile_save')` usage. This is the main frontend autosave boundary.
- `src/crosshook-native/src/types/profile.ts`: TypeScript mirror of the profile schema; must grow the new optimization subsection.
- `src/crosshook-native/src/types/launch.ts`: TypeScript request contract passed to `validate_launch`, `launch_game`, and `launch_trainer`.
- `src/crosshook-native/src-tauri/src/commands/profile.rs`: Thin Tauri command layer over `ProfileStore`; likely home for a future `profile_save_launch_optimizations` command.
- `src/crosshook-native/src-tauri/src/commands/launch.rs`: Thin Tauri command layer that validates requests, resolves helper scripts, spawns child processes, and streams logs.
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: Rust source of truth for persisted profile shape.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: TOML storage implementation, name validation, and round-trip tests.
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: Rust `LaunchRequest` type plus validation rules keyed off launch method.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: Actual process construction for `proton_run`, `steam_applaunch`, and native launches. This is the correct integration point for env/wrapper translation.
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: Shared environment rehydration helpers. Because commands start from `env_clear()`, any new supported optimization env vars must be set explicitly here or in the caller.
- `src/crosshook-native/runtime-helpers/steam-launch-helper.sh`: Existing Steam helper boundary. Relevant mainly as a contrast, because the required feature scope is `proton_run` and should not be designed around Steam helper limitations.

## Configuration

- Profile files live in `~/.config/crosshook/profiles`, resolved through `directories::BaseDirs`.
- Launch requests currently derive `steam_client_install_path` in React using compatdata or `default_steam_client_install_path`.
- `proton_run` commands start from `env_clear()` and then repopulate only approved host variables and Proton variables; this makes env-based optimizations deterministic but requires explicit allowlisting.
- Host session variables preserved by `apply_host_environment()` are `HOME`, `USER`, `LOGNAME`, `SHELL`, `PATH`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, and `DBUS_SESSION_BUS_ADDRESS`.
- Proton runtime variables currently set by CrossHook for direct Proton launch are `WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, and sometimes `STEAM_COMPAT_CLIENT_INSTALL_PATH`.
- Existing launch logs are written to `/tmp/crosshook-logs`, and both `launch_game` and `launch_trainer` stream those logs back to the UI.
- Runtime helper scripts are resolved from bundled Tauri resources or the development `runtime-helpers/` directory.
- Wrapper-based options such as MangoHud, `gamemoderun`, and `game-performance` are host binary dependencies and should be validated against `PATH` or explicit discovery before launch.
- The current full-save path in `useProfile.persistProfileDraft()` also updates settings and recent-files metadata and reloads the profile after saving. That makes it unsuitable for fine-grained checkbox autosave without a narrower persistence path.
