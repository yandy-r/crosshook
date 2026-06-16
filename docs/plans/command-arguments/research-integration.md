# Integration Research: command-arguments

## API / IPC Surface

- `src/crosshook-native/src-tauri/src/lib.rs`: Tauri command registration already exposes the launch/profile surfaces this feature should extend: `launch_game`, `launch_trainer`, `validate_launch`, `preview_launch`, `build_steam_launch_options_command`, `profile_load`, `profile_save`, `profile_save_launch_optimizations`, and bundled/manual optimization preset commands.
- `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`: thin IPC layer for validation, preview, and Steam launch-option generation. `preview_launch` enriches `LaunchRequest` with resolved umu GAMEID before calling `crosshook_core::launch::build_launch_preview`; `build_steam_launch_options_command` currently accepts optimization IDs, custom env vars, and optional Gamescope config.
- `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`: thin IPC launch execution entry point. It takes `LaunchRequest`, resolves umu GAMEID in the command layer, validates, records launch metadata, and delegates actual process construction/execution to `crosshook-core`.
- `src/crosshook-native/src-tauri/src/commands/profile/lifecycle.rs`: `profile_load` / `profile_save` pass `GameProfile` across IPC and persist TOML through `ProfileStore`. New per-profile command arguments fit this path naturally.
- `src/crosshook-native/src-tauri/src/commands/profile/optimizations.rs`: precedent for specialized launch-section mutation commands and bundled preset application. A command-argument autosave command could mirror `profile_save_launch_optimizations`, but a normal `profile_save` / launch-section save may be simpler if arguments live with the same one-page launch editor.
- `src/crosshook-native/src/types/profile.ts` and `src/crosshook-native/src/types/launch.ts`: TypeScript mirrors for `GameProfile.launch` and `LaunchRequest`. Both must gain the same structured argument shape so saved profiles, preview, launch, mocks, and export flows stay aligned.
- `src/crosshook-native/src/utils/launch.ts`: builds `LaunchRequest` from a loaded `GameProfile`; this is the frontend handoff point for adding selected curated argument IDs and custom argument values.
- `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`: current Steam-only copy/paste surface calls `build_steam_launch_options_command` and expects the generated line to end with `%command%`. Command arguments would be appended after `%command%` here.
- `src/crosshook-native/src/components/LaunchSubTabs.tsx` and `src/crosshook-native/src/components/launch-subtabs/*`: current launch UX is tabbed inside one launch page. The user scope says one page and not a separate launch-options flow, so integrate command arguments into the existing launch configuration surface, likely adjacent to Optimizations/Environment rather than a new route.
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts` and profile mock handlers: browser dev mode mocks implement `preview_launch`, `build_steam_launch_options_command`, and profile mutations. Any new IPC command or DTO field needs matching mock support.

## Database Schema

- Current schema is v24 per `AGENTS.md`; migrations live in `src/crosshook-native/crates/crosshook-core/src/metadata/migrations/`.
- Optimization catalog storage exists in SQLite:
  - `migrate_11_to_12` creates `optimization_catalog` with functional fields (`env_json`, `wrappers_json`, conflicts, required binary) and UI metadata.
  - `src/crosshook-native/crates/crosshook-core/src/metadata/optimization_catalog_store.rs` persists the merged in-memory optimization catalog with transactional delete/insert.
- Bundled optimization presets and per-profile preset metadata exist in SQLite:
  - `migrate_9_to_10` creates `bundled_optimization_presets` and `profile_launch_preset_metadata`.
  - `src/crosshook-native/crates/crosshook-core/src/metadata/preset_store.rs` lists bundled presets and records preset origin metadata.
- Profile launch choices themselves are not stored in SQLite. `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs` stores `launch.optimizations`, `launch.presets`, `launch.active_preset`, `launch.custom_env_vars`, Gamescope, MangoHud, and network isolation in profile TOML.
- Recommendation: do not add a SQLite table for user-selected command arguments. Persist selected curated IDs and custom argument tokens in profile TOML under `launch`, because they are user-editable profile configuration like `custom_env_vars` and `optimizations`.
- A SQLite command-argument catalog is optional, not required for the first implementation. The closest reusable pattern is the optimization catalog: embedded TOML asset plus optional user override plus community tap merge, then optionally persisted to SQLite for diagnostics/search. If the UI can consume the catalog directly from `crosshook-core`, the first version can avoid a schema migration.

## External Services and Host Tools

- Steam launch options:
  - `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs` builds `KEY=value wrapper %command%` from the same optimization directive resolver used by `proton_run`.
  - Steam-style game arguments should be appended after `%command%`, e.g. `%command% --vulkan --launcher-skip`.
  - Existing `escape_steam_token` is a useful precedent, but argument escaping should be treated separately from env-value escaping and tested with spaces, quotes, `$`, `;`, `|`, `&`, `<`, `>`, and empty values.
- Proton / umu:
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs` builds the effective game command and currently appends only `normalized_game_path` after the Proton/umu program setup.
  - `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs` constructs the wrapper chain, Gamescope chain, Flatpak host spawn, and `proton run` / `umu-run` executable boundary. It intentionally omits `run` when `use_umu` is true.
  - Command arguments should be appended after the normalized game executable path in `build_proton_game_command`; this covers both direct Proton and umu because `umu-run` swaps the program path but still receives the game executable as argv.
- Host gateway:
  - `src/crosshook-native/crates/crosshook-core/src/platform/mod.rs` re-exports the host gateway helpers.
  - `src/crosshook-native/crates/crosshook-core/src/platform/gateway.rs` requires host tools to go through `host_command*` / `host_std_command*`; Flatpak env handling uses `flatpak-spawn --host --clear-env`, `--directory`, and a temporary env file for user custom env vars.
  - Adding game arguments should only add `.arg(...)` entries to commands already produced by the gateway-aware builders. Do not introduce direct `Command::new("proton")`, `Command::new("umu-run")`, `Command::new("gamescope")`, etc.
- Native:
  - `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/native.rs` launches the native game path directly with custom env vars and working directory.
  - If command arguments are framed as generic "game arguments", native can support custom arguments by appending argv after the native executable. Curated Windows/Proton-oriented argument presets should use method applicability metadata so the UI can hide or warn for native.
- External network services are not required. Existing umu GAMEID lookup and ProtonUp catalogs are unrelated except that preview/launch already resolve umu state before command construction.

## Internal Services

- `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`: primary profile model extension point. Add a launch subsection for command arguments, for example selected predefined argument IDs plus custom argument tokens.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`: `LaunchRequest` needs the effective argument selection so validation, preview, Steam command generation, and execution receive the same data.
- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: optimization catalog precedent. A command-argument catalog should likely be a sibling module rather than overloading `OptimizationEntry`, because arguments are argv tokens, not env/wrapper directives. Still mirror `id`, `label`, `description`, `help_text`, `category`, `advanced`, `community`, `conflicts_with`, and `applicable_methods`.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/directives.rs`: optimization validation resolves known IDs, duplicates, conflicts, method support, and required wrapper binaries. Command arguments need analogous validation for unknown/duplicate curated argument IDs, conflicts, and method applicability, but probably no `required_binary`.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/validation.rs`: centralized validation. Add command-argument validation here so `validate_launch`, `preview_launch`, and real execution agree. Custom arguments should reject NUL bytes and either store already-tokenized argv entries or reject ambiguous raw shell strings.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`: preview computes directives, environment, wrappers, effective command, and Steam launch options. Add resolved command-argument data here so previews show exactly what will launch.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`: effective command string currently appends only game path for Proton/umu and delegates Steam options to `build_steam_launch_options_command`. Append escaped display tokens after the game path for Proton/umu/native and after `%command%` for Steam.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`: runtime execution append point for Proton/umu game arguments after `command.arg(normalized_game_path.trim())`.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/native.rs`: runtime execution append point for native arguments after creating the game command.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_trainer.rs` and Steam trainer helpers: do not apply game arguments to trainer-only launches unless a later feature explicitly introduces trainer arguments.
- Launcher export/preview flows build `LaunchRequest` from frontend state (`src/crosshook-native/src/hooks/useLauncherExport.ts`), so extending `LaunchRequest` should automatically expose arguments to script/desktop previews if backend export builders use the same core launch command functions. Verify launcher exporters do not have a parallel string builder that would omit the new field.

## Configuration and Persistence Boundary

- TOML settings (`settings.toml`):
  - Global defaults such as a "default argument preset for new profiles" could live in `AppSettingsData`, but the feature scope is per-profile Steam-style command arguments. Do not put selected per-game arguments in global settings.
  - If a future global default is added, follow `default_bundled_optimization_preset_id` in `src/crosshook-native/crates/crosshook-core/src/settings/types.rs`.
- Profile TOML:
  - Persist user-selected curated command argument IDs and custom argument tokens under `launch`.
  - Suggested shape: `launch.command_arguments.selected_argument_ids = []` and `launch.command_arguments.custom_args = []`, or equivalent nested Serde struct with `skip_serializing_if = is_empty`.
  - Store custom arguments as a list of argv tokens, not a single raw shell string. This avoids shell splitting differences between Steam display strings, Proton/umu `Command::arg`, and native `Command::arg`.
  - If UX accepts a pasted raw line, parse it into tokens before saving and surface parse errors; do not defer parsing to launch time.
- SQLite metadata:
  - Existing `launch_operations` can continue recording launch method/status/diagnostics; no new persisted launch-history columns are required unless product wants historical reporting of exact arguments used.
  - Existing `optimization_catalog` and preset metadata are useful precedents but not a required storage target for this feature's user selections.
  - A future `command_argument_catalog` table would be operational/catalog metadata, not user preference. Add it only if command argument catalogs need metadata-store indexing, offline cache behavior, or community tap diagnostics.
- Runtime-only:
  - Resolved effective arguments in `LaunchRequest`, `LaunchPreview`, and generated command objects are runtime-only.
  - Escaped Steam launch-options strings are derived output and should not be stored as source of truth.
  - umu GAMEID resolution remains runtime/enriched metadata behavior and should not be coupled to command argument persistence.

## Integration Recommendations

- Model command arguments as a sibling to launch optimizations, not as another optimization directive. Optimizations modify env/wrappers; command arguments modify game argv after the executable (`proton run game.exe <args>`, `umu-run game.exe <args>`, `%command% <args>`).
- Create a dedicated command-argument catalog in `crosshook-core`, likely backed by `src/crosshook-native/assets/default_command_argument_catalog.toml`, with IDs such as `force_vulkan` mapping to one or more argv tokens. Reuse optimization-catalog concepts: validation, conflicts, categories, advanced/community flags, and method applicability.
- Keep `src-tauri` thin. Add at most thin commands such as `get_command_argument_catalog` or `profile_save_command_arguments`; all parsing, validation, catalog resolution, and command construction should live in `crosshook-core`.
- Apply arguments to `steam_applaunch` and `proton_run`; for `proton_run`, this automatically includes umu because umu is selected inside the same builder. Consider supporting native custom arguments too, but gate curated presets by `applicable_methods` so Proton/Steam-specific entries do not appear for native launches.
- Append order should be deterministic: optimization/env/wrappers first, `%command%` or game executable path next, curated argument tokens in catalog order, then custom argument tokens in user order. This mirrors optimization directive ordering while letting user custom entries override/extend by position where games care about last occurrence.
- Update preview and Steam copy/paste generation in the same change. Tests should cover `build_steam_launch_options_command(...args...) == "%command% --vulkan"`, Proton command argv after `game.exe`, umu command argv without inserting `run`, Gamescope wrapper ordering, and trainer-only launches not receiving game args.
- Add frontend controls inside the existing launch page/subtab system. Avoid a separate route. A practical shape is to extend the Optimizations tab into a broader "Launch Options" panel or add an Arguments section beside optimizations on the same launch configuration page, while leaving the Steam copy/paste preview as derived output for Steam profiles.
- Update mocks and test fixtures together with DTOs. Browser dev mode relies on `src/crosshook-native/src/lib/mocks/handlers/launch.ts`; raw `invoke()` bypasses mocks, so new UI should continue using `callCommand`.
- Validation should reject unknown curated IDs, duplicates, conflicts, unsupported methods, NUL bytes in custom tokens, and excessive token length/count. Do not reject common CLI punctuation such as `--flag=value`, `+set`, `-dx11`, or paths with spaces when stored as structured tokens.
- Do not introduce shell command string concatenation for real launches. Real Proton/umu/native execution should use `Command::arg` for each token. Only Steam launch-options output requires string escaping because Steam expects a pasted command line.
