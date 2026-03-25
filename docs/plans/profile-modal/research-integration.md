# Integration Research: profile-modal

## API Endpoints

### Existing Related Endpoints

- `install_default_prefix_path(profile_name: String) -> Result<String, String>`: Tauri command used by [`useInstallGame`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts) to derive the default prefix under the local data directory before install. Frontend invokes it as `invoke<string>('install_default_prefix_path', { profileName })`.
- `validate_install_request(request: InstallGameRequest) -> Result<(), String>`: Tauri command used by [`useInstallGame`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts) before launching the installer. Validation errors are surfaced as strings and mapped back to individual install form fields in the hook.
- `install_game(request: InstallGameRequest) -> Result<InstallGameResult, String>`: Tauri command used by [`useInstallGame`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts) to run the installer through Proton, create a reviewable `GameProfile`, and return candidate executable paths plus a helper log path.
- `profile_list() -> Result<Vec<String>, String>`: Tauri command used by [`useProfile`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts) to refresh the available profile names after save and on initial load.
- `profile_load(name: String) -> Result<GameProfile, String>`: Tauri command used by [`useProfile`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts) to load the persisted TOML profile after save and when selecting an existing profile.
- `profile_save(name: String, data: GameProfile) -> Result<(), String>`: Tauri persistence command already used by [`useProfile`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts). This is the save path the modal should reuse.
- `settings_load() -> Result<AppSettingsData, String>` and `settings_save(data: AppSettingsData) -> Result<(), String>`: Tauri commands called inside `useProfile.syncProfileMetadata` to update `last_used_profile` after save.
- `recent_files_load() -> Result<RecentFilesData, String>` and `recent_files_save(data: RecentFilesData) -> Result<(), String>`: Tauri commands called inside `useProfile.syncProfileMetadata` to update recent game, trainer, and DLL paths after save.
- `list_proton_installs(steam_client_install_path?: String) -> Result<Vec<ProtonInstall>, String>`: Tauri command used by both [`InstallGamePanel`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx) and [`ProfileEditor`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx) to populate Proton-path selectors in the shared editing surface.
- `@tauri-apps/plugin-dialog.open(...) -> Promise<string | string[] | null>`: frontend file picker used by install and profile field groups for browsing executables and directories. The modal will inherit this dependency if it reuses those field groups.

### Route Organization

All frontend/backend integration is through Tauri `invoke(...)` calls from React hooks and components. Commands are registered centrally in [`src-tauri/src/lib.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs) and implemented in grouped modules:

- Install commands in [`src-tauri/src/commands/install.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/install.rs) wrap blocking core install services via `tauri::async_runtime::spawn_blocking`.
- Profile commands in [`src-tauri/src/commands/profile.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs) call the managed `ProfileStore` directly.
- Settings and recent-files commands in [`src-tauri/src/commands/settings.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/settings.rs) call managed TOML stores directly.
- Steam-adjacent discovery commands in [`src-tauri/src/commands/steam.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs) support Proton-path selection but are not part of the save/install core flow.

The current UI seam for the future modal is in [`InstallGamePanel`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx) and [`ProfileEditorView`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx):

- Today `InstallGamePanel` exposes `onReviewGeneratedProfile(profileName, profile)`.
- Today `ProfileEditorView.handleInstallReview` calls `hydrateProfile(profileName, generatedProfile)` and then immediately sets `editorTab` to `'profile'`.
- For `profile-modal`, that direct handoff is the place to replace with `InstallProfileReviewPayload` and modal session state.

Relevant data-shape relationships for that handoff:

- [`InstallGameRequest`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts) and the Rust `InstallGameRequest` in [`install/models.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs) have the same snake_case transport shape.
- [`InstallGameResult`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts) and the Rust `InstallGameResult` in [`install/models.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs) also align directly over Tauri IPC.
- `InstallGameResult.profile` is a `GameProfile`; `useInstallGame` additionally derives `reviewProfile` from that profile plus the current `installed_game_executable_path`, keeping `runtime.working_directory` synchronized with the chosen executable.
- `candidateOptions` is a frontend-only derived array built from `InstallGameResult.discovered_game_executable_candidates`.
- The future `InstallProfileReviewPayload` should be assembled from `result.profile_name`, current `reviewProfile`, `candidateOptions`, `result.helper_log_path`, and `result.message`, not just the raw `result.profile`, because install-panel edits to the final executable already mutate `reviewProfile`.
- The future `ProfileReviewSession` is modal-local state. That file does not exist yet in [`src/types`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types), so implementing the modal will require adding it and likely exporting it from [`src/types/index.ts`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/index.ts) if shared broadly.
- `GameProfile` is the common contract across install result, modal draft state, `useProfile`, Tauri IPC, and TOML persistence. The TypeScript shape in [`src/types/profile.ts`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts) matches the Rust serde model in [`profile/models.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs), including nested `game`, `trainer`, `injection`, `steam`, `runtime`, and `launch` sections.
- Save eligibility today is enforced twice in the frontend: `ProfileEditorView` only enables save when `profileName` and `profile.game.executable_path` are non-empty, and `useProfile.validateProfileForSave` rejects missing `game.executable_path`.
- `useProfile.saveProfile()` normalizes the draft before save, persists with `profile_save`, updates settings/recent-files metadata, refreshes the profile list, and reloads the saved profile. That means the existing save path already selects the saved profile after success; the modal orchestration only needs to switch the visible editor tab to `'profile'` after the promise resolves.

## Database

There is no database in this flow.

Persistence is filesystem-backed TOML:

- Profiles are stored by `ProfileStore` under `~/.config/crosshook/profiles/*.toml` in [`profile/toml_store.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs).
- App settings are stored under `~/.config/crosshook/settings.toml` by `SettingsStore` in [`settings/mod.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs).
- Recent paths are stored under `~/.local/share/crosshook/recent.toml` by `RecentFilesStore` in [`settings/recent.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/recent.rs).

## External Services

There are no network services or remote APIs in this flow.

Relevant platform and third-party integrations are local:

- Proton executable invocation: core install service launches the installer through a locally selected Proton binary using runtime helpers in [`install/service.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs).
- Local filesystem access: install validation checks existence and type of installer, trainer, Proton, prefix, and optional final executable paths; save writes TOML profiles to the user config directory.
- Tauri dialog plugin: existing install/profile editors use `@tauri-apps/plugin-dialog` for browse actions, so a modal reusing those field groups inherits the same capability and UX dependency.
- React DOM portal: the feature spec recommends `createPortal`; this is already available through the existing `react-dom` dependency in [`package.json`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/package.json).

## Internal Services

Internal modules that matter for `profile-modal`:

- [`useInstallGame`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts): owns install request state, validation, async install execution, stage transitions, `reviewProfile`, and `candidateOptions`.
- [`InstallGamePanel`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx): current UI owner of the install form and install-review handoff callback.
- [`useProfile`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts): owns editable profile state, normalization, validation, persistence, metadata sync, profile reload, and selection.
- [`ProfileEditorView`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx): owns the current `editorTab` state and is the right frontend coordination point for opening a modal and switching back to the Profile tab after successful save.
- [`crosshook_core::install::service`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs): validates install requests, provisions the prefix directory, runs the installer, discovers candidate executables, and builds the initial reviewable `GameProfile`.
- [`crosshook_core::install::models`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs): defines the IPC-stable Rust request/result shapes and the logic for building the initial `GameProfile` from install inputs.
- [`ProfileStore`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs): final persistence backend for `profile_save`.
- [`SettingsStore`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs) and [`RecentFilesStore`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/recent.rs): side-effect stores updated after successful save.
- [`commands/steam.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs) plus core Steam Proton discovery: supports the existing Proton selectors used by both the install panel and the profile editor.

Behavioral constraints surfaced by these services:

- `install_game` always returns `needs_executable_confirmation: true`, even when it can prefill an executable. The modal therefore has to support both complete and incomplete review states.
- `useInstallGame.setInstalledExecutablePath()` updates both `reviewProfile.game.executable_path` and `reviewProfile.runtime.working_directory`; the modal should preserve that derived behavior when opening from an edited install session.
- `useProfile.saveProfile()` is stateful around the hook’s current `profileName` and `profile`. A modal that keeps an isolated `ProfileReviewSession` draft will need an explicit handoff into `useProfile` state before invoking the existing save path, or a thin helper added inside `useProfile` that persists an explicit `(name, profile)` pair through the same normalization and metadata-sync logic.

## Configuration

- Tauri window and viewport baseline: [`tauri.conf.json`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/tauri.conf.json) configures the main window at `1280x800`, which matches the feature spec’s viewport target for a large modal with internal scrolling.
- Tauri capabilities: [`capabilities/default.json`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/capabilities/default.json) grants `core:default` and `dialog:default`. `core:default` is what allows `invoke(...)`; `dialog:default` is required for file/directory pickers inside reused field groups.
- Tauri plugins registered in [`src-tauri/src/lib.rs`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs): `tauri_plugin_dialog`, `tauri_plugin_fs`, and `tauri_plugin_shell` are initialized globally. The modal’s direct dependency is dialog; install execution and related app behavior also rely on shell/fs availability elsewhere in the app.
- Home-directory requirement: `ProfileStore`, `SettingsStore`, `RecentFilesStore`, and default-prefix resolution all depend on `directories::BaseDirs`. If the home directory cannot be resolved, app startup or install prefix derivation fails.
- Prefix and profile naming constraints: both install validation and profile persistence reject invalid profile names. Install prefix defaulting slugifies the profile name into `~/.local/share/crosshook/prefixes/<slug>`, while persisted profile files use the unslugified validated name as `<name>.toml`.
- Steam/Proton discovery inputs: `list_proton_installs` can use an explicit `steam_client_install_path`, otherwise it falls back to `STEAM_COMPAT_CLIENT_INSTALL_PATH`, then standard Steam install roots under the current user home directory.
