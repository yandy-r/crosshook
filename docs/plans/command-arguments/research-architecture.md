# Architecture Research: command-arguments

## System Overview

CrossHook is a native Linux Tauri v2 desktop app with launch orchestration concentrated in `src/crosshook-native/crates/crosshook-core`; Tauri commands are thin IPC wrappers and the React/TypeScript frontend edits profile-backed launch state. The command-arguments feature should model per-profile game arguments as TOML profile data, expose them through `LaunchRequest`, and apply them in both generated Steam launch-options strings and direct Proton/umu command construction so preview, copy/paste, and real launch behavior stay aligned.

## Relevant Components

- `src/crosshook-native/crates/crosshook-core/src/profile/models/launch.rs`: Defines `LaunchSection`, existing `optimizations`, `presets`, `custom_env_vars`, gamescope, MangoHud, and collection-default override fields; this is the natural home for persisted profile command-argument selections.
- `src/crosshook-native/crates/crosshook-core/src/profile/models/profile.rs`: Merges effective profile layers and collection defaults; any argument field added under `LaunchSection` needs merge semantics here if collection defaults should affect arguments.
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store/store.rs`: Loads/saves profile TOML, normalizes preset state, validates optimization IDs, and provides narrow save helpers; add a narrow save path if arguments autosave separately.
- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: Existing curated optimization catalog pattern: embedded TOML, optional community/user overrides, validation, UI metadata, and process-global catalog.
- `src/crosshook-native/assets/default_optimization_catalog.toml`: Default curated optimization entries with labels, categories, conflicts, required binaries, and applicable methods; command-argument presets can mirror this format with argument-specific fields.
- `src/crosshook-native/crates/crosshook-core/src/metadata/optimization_catalog_store.rs`: Persists optimization catalog rows to SQLite for metadata/catalog indexing. Use only if command-argument catalog needs metadata caching; per-profile argument choices should not live in SQLite.
- `src/crosshook-native/crates/crosshook-core/src/launch/request/models.rs`: IPC/backend `LaunchRequest` DTO; currently carries method, game/trainer paths, `optimizations`, `custom_env_vars`, umu settings, gamescope, MangoHud, and hooks.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations/steam_options.rs`: Builds the Steam Launch Options line as `env wrappers gamescope %command%`; this is where Steam-style game arguments must be appended after `%command%`.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/proton_game.rs`: Builds real direct Proton or `umu-run` game commands and currently appends only `game_path`; argument tokens should be appended immediately after the game executable here.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/umu.rs`: Decides whether the Proton game path uses `umu-run` and populates `GAMEID`/`STORE`; command arguments apply to the final target command regardless of direct Proton vs umu.
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers/proton_command.rs`: Central host-command construction helpers that route Flatpak host execution through `platform` helpers; keep host-tool process creation here and only add target args through safe `Command::arg` calls.
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner/native.rs`: Builds native game commands and applies custom env; decide explicitly whether command arguments also apply to native Linux executables.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/command.rs`: Builds the human-readable effective command shown in previews; must include game arguments wherever real command construction includes them.
- `src/crosshook-native/crates/crosshook-core/src/launch/preview/builder.rs`: Assembles preview environment, wrappers, effective command, and Steam launch options; must thread arguments into both `effective_command` and `steam_launch_options`.
- `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`: Thin IPC for `preview_launch` and `build_steam_launch_options_command`; needs signature updates if Steam options generation accepts argument selections/custom args.
- `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`: Thin IPC for `launch_game`/`launch_trainer`; no business logic should move here, but it will receive the expanded `LaunchRequest`.
- `src/crosshook-native/src/utils/launch.ts`: Builds frontend `LaunchRequest` from `GameProfile`; must copy profile command-argument state into the IPC payload.
- `src/crosshook-native/src/types/profile.ts`: Frontend `GameProfile` type and normalization defaults; add argument fields under `launch` and normalize older profiles to empty selections/custom args.
- `src/crosshook-native/src/types/launch.ts`: Frontend `LaunchRequest` type; must match the Serde DTO shape in Rust.
- `src/crosshook-native/src/components/LaunchSubTabs.tsx`: One-page launch configuration surface shared by legacy launch UI and hero detail launch UI; command arguments should be integrated here rather than as a new page/route.
- `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`: Established curated-toggle UX for selectable launch optimizations; command arguments can reuse this grouped, catalog-driven selection model.
- `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`: Current Steam copy/paste panel calls `build_steam_launch_options_command`; update it to include game arguments after `%command%` or fold this copy/paste preview into the unified argument/optimization surface.
- `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts`: Assembles profile launch state and handlers for `LaunchSubTabs`; add argument state and save handlers here.
- `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosave.ts`: Existing autosave chain for launch optimizations/gamescope/MangoHud; a separate argument autosave can share the serialized launch write queue to avoid clobbering concurrent launch-section saves.
- `src/crosshook-native/src/hooks/profile/useProfileLaunchAutosaveEffects.ts`: Optimization autosave is a narrow `profile_save_launch_optimizations` IPC write; use as the pattern for an arguments-specific save effect.
- `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`: Existing row-editor pattern for user custom data with validation; useful for custom free-form argument rows/tokens.
- `src/crosshook-native/src/components/launch-subtabs/useTabVisibility.ts`: Controls visible launch subtabs by method; argument controls should appear with the one-page launch surface for `steam_applaunch` and `proton_run`, and only for `native` if the product decision includes native executable args.

## Data Flow

Profile launch settings are stored in per-profile TOML through `GameProfile.launch`. The frontend loads a profile, normalizes it in `normalizeProfileForEdit`, and exposes launch controls through `ProfileContext`, `useLaunchSubTabsProps`, and `LaunchSubTabs`. Launch execution builds a `LaunchRequest` in `src/crosshook-native/src/utils/launch.ts`; `LaunchStateContext` passes that request to `useLaunchState`, which invokes `validate_launch`, `launch_game`, `launch_trainer`, or `preview_launch`.

For existing optimizations, selected IDs live in `launch.optimizations.enabled_option_ids`, are validated against the global optimization catalog, and resolve in core to env pairs and wrapper tokens. Custom env vars live in `launch.custom_env_vars`, are added to `LaunchRequest.custom_env_vars`, and are merged after optimization env so user values win.

Command arguments should follow the same broad flow, but they are not env vars or process wrappers. The recommended data model is profile TOML under `launch`, for example a curated ID list plus custom argument tokens. Those fields should be copied into `LaunchRequest`, validated/sanitized in `crosshook-core`, and rendered into command tails. For Steam launch options, arguments belong after `%command%`; for direct Proton and umu, arguments belong after the target game executable path in the spawned command. Preview code must use the same resolver so the displayed command, Steam copy line, and real launch do not diverge.

Persistence classification: selected predefined arguments and custom user arguments are user-editable preferences, so they belong in profile TOML, not SQLite. A curated argument catalog can be embedded static TOML like the optimization catalog; SQLite should only be considered for a metadata mirror/cache if the app needs catalog indexing, not as the source of profile selections. Runtime resolved command strings and preview output are ephemeral runtime state.

## Integration Points

- Add core models under `profile/models/launch.rs`: a `LaunchArgumentsSection` such as `enabled_argument_ids` plus `custom_args`, with `is_empty()` and serde defaults for backward compatibility.
- Add a launch-argument catalog in `crosshook-core` if curated arguments need labels/categories/help/conflicts. Mirror `launch/catalog.rs` rather than overloading optimization entries, because argument entries produce argv tokens instead of env/wrappers.
- Add resolver functions in `crosshook-core` that map selected argument IDs plus custom args into an ordered `Vec<String>`. Keep validation in core: trim empty values, reject NUL/control characters, detect duplicate/conflicting curated IDs, and preserve token boundaries by storing custom args as an array rather than parsing a shell string.
- Extend `LaunchRequest` in `launch/request/models.rs` and `src/types/launch.ts` with argument data. Update `buildProfileLaunchRequest` so launches, previews, and Steam option generation receive the same state.
- Update `build_steam_launch_options_command` to accept resolved game arguments and output `... %command% <args>`. Reuse `escape_steam_token` for each argument token so spaces/metacharacters are preserved.
- Update `build_proton_game_command` so both direct Proton and `umu-run` append game args after `normalized_game_path`. This applies equally to umu because `program_path` switches from Proton to `umu-run` before the game path is appended.
- Decide whether `build_native_game_command` receives arguments. Architecturally it can append them after the native executable with `Command::arg`; product scope should decide because the user examples are Windows/Steam game args.
- Keep trainer paths separate. Current trainer builders append trainer executable/staged path and should not inherit game command arguments unless a future explicit trainer-argument feature is added.
- Update `preview/command.rs` and `preview/builder.rs` to include resolved args in effective command and Steam launch options. Add focused tests in `launch/preview/tests/command_string.rs` and `launch/optimizations/steam_options.rs` or a new argument module test file.
- Add thin Tauri IPC only where needed: catalog read command such as `get_command_argument_catalog`, a narrow profile save command if autosaving separately, and an updated `build_steam_launch_options_command` signature.
- Frontend UX should stay in the existing launch page surface. Prefer a new panel inside `LaunchSubTabs` or an expansion of `OptimizationsTabContent`/`LaunchOptimizationsPanel` so curated toggles and custom args are visible together on one page; avoid adding a separate route or making users jump to the existing Steam Options tab for editing.
- Autosave should reuse the existing launch-section write queue in `useProfileLaunchAutosave.ts` to avoid races with optimization/gamescope/MangoHud saves. For unsaved profiles, update local draft state and persist with the full profile save path later.
- If collection launch defaults should include arguments, extend `CollectionDefaultsSection` and `effective_profile_with`; if not, document that arguments remain profile-specific only.

## Key Dependencies

- Tauri v2 IPC with `#[tauri::command]`, Serde DTOs, and frontend `callCommand()` adapter.
- Rust `tokio::process::Command` for launch execution; Flatpak host-tool execution must continue through `crosshook-core/src/platform.rs` helpers used by runtime command builders.
- `toml`, `serde`, and profile TOML storage for user-editable launch argument preferences.
- `rusqlite`/`MetadataStore` only for optional catalog mirrors or cached metadata; not for per-profile argument selections.
- React 18 + TypeScript, Radix Tabs, existing `crosshook-*` BEM-like CSS classes, `LaunchSubTabs`, and profile autosave hooks.
- Existing optimization catalog and UI utilities provide patterns for curated labels, categories, conflicts, capability gating, and grouped toggle rendering.
