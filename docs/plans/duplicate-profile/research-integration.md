# Integration Research: Duplicate Profile

## Overview

Profile duplication requires coordination across four layers: Rust `ProfileStore` (TOML persistence), Tauri IPC commands, the React `useProfile` hook + `ProfileContext`, and the launcher lifecycle. The operation is fundamentally a **load-then-save-with-new-name** — the `GameProfile` struct is value-typed, serde-derived, and `Clone`, so a backend `duplicate` method can clone the data, generate a unique name, and write a new TOML file. No launcher files need to be created or modified during duplication because launchers are only exported on explicit user action via the `LauncherExport` component.

---

## API Endpoints

### Tauri IPC Commands

#### Existing Profile Commands

Registered in `src-tauri/src/lib.rs:91-97`:

| Command                             | Signature                                                                                              | Purpose                                                         |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| `profile_list`                      | `(State<ProfileStore>) -> Result<Vec<String>, String>`                                                 | Lists all `.toml` file stems in `~/.config/crosshook/profiles/` |
| `profile_load`                      | `(name: String, State<ProfileStore>) -> Result<GameProfile, String>`                                   | Deserializes `{name}.toml` into `GameProfile`                   |
| `profile_save`                      | `(name: String, data: GameProfile, State<ProfileStore>) -> Result<(), String>`                         | Serializes `GameProfile` to `{name}.toml`                       |
| `profile_delete`                    | `(name: String, State<ProfileStore>) -> Result<(), String>`                                            | Deletes profile + best-effort launcher cleanup                  |
| `profile_rename`                    | `(old_name: String, new_name: String, State<ProfileStore>) -> Result<(), String>`                      | Renames `.toml` file (fs::rename)                               |
| `profile_save_launch_optimizations` | `(name: String, optimizations: LaunchOptimizationsPayload, State<ProfileStore>) -> Result<(), String>` | Merges only `launch.optimizations` into existing profile        |
| `profile_import_legacy`             | `(path: String, State<ProfileStore>) -> Result<GameProfile, String>`                                   | Converts `.profile` files to TOML format                        |

#### IPC Patterns

- All commands map errors with `error.to_string()` — frontend receives `String` errors
- `ProfileStore` is managed as Tauri state via `.manage(profile_store)` (line 62)
- Commands access store via `State<'_, ProfileStore>` parameter injection
- Command names are `snake_case` and match frontend `invoke()` call strings exactly
- `GameProfile` crosses the IPC boundary via serde `Serialize`/`Deserialize` derives

#### New Command Needed

A `profile_duplicate` command should follow existing patterns:

```rust
#[tauri::command]
pub fn profile_duplicate(
    source_name: String,
    store: State<'_, ProfileStore>,
) -> Result<String, String> {
    store.duplicate(&source_name).map_err(map_error)
}
```

The command should return the generated new name so the frontend can select it. Must be registered in `lib.rs` `invoke_handler` array.

---

## TOML Persistence

### Storage Location

- Base path: `~/.config/crosshook/profiles/` (resolved via `directories::BaseDirs::config_dir()`)
- File naming: `{profile-name}.toml`
- `ProfileStore::with_base_path()` exists for testing with temp dirs

### ProfileStore Methods (toml_store.rs)

| Method                                 | Behavior                                                                            |
| -------------------------------------- | ----------------------------------------------------------------------------------- |
| `load(name)`                           | Validates name, reads `{name}.toml`, deserializes via `toml::from_str`              |
| `save(name, profile)`                  | Validates name, creates dir if needed, writes `toml::to_string_pretty`              |
| `list()`                               | Reads dir entries, filters `.toml` extension, returns sorted file stems             |
| `delete(name)`                         | Validates name, checks existence, removes file                                      |
| `rename(old, new)`                     | Validates both names, checks old exists, `fs::rename` (overwrites target if exists) |
| `save_launch_optimizations(name, ids)` | Load-modify-save cycle targeting only `launch.optimizations`                        |
| `import_legacy(path)`                  | Converts legacy `.profile` format to `GameProfile` and saves                        |
| `profile_path(name)`                   | Internal: validates name, returns `base_path.join("{name}.toml")`                   |

### Name Validation (validate_name)

Located at `toml_store.rs:189-214`. Rules:

- Rejects empty strings, `.`, `..`
- Rejects absolute paths
- Rejects any of: `< > : " / \ | ? *`
- Trims whitespace before validation

**Implication for duplication**: Generated names like `"My Profile (Copy)"` or `"My Profile (Copy 2)"` are valid — parentheses, spaces, and digits are all allowed.

### GameProfile Struct (models.rs)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct GameProfile {
    pub game: GameSection,        // name, executable_path
    pub trainer: TrainerSection,  // path, kind/type, loading_mode
    pub injection: InjectionSection, // dll_paths: Vec<String>, inject_on_launch: Vec<bool>
    pub steam: SteamSection,      // enabled, app_id, compatdata_path, proton_path, launcher: LauncherSection
    pub runtime: RuntimeSection,  // prefix_path, proton_path, working_directory (skip_serializing_if empty)
    pub launch: LaunchSection,    // method, optimizations: LaunchOptimizationsSection
}
```

Key sub-structs:

- **GameSection**: `name: String`, `executable_path: String`
- **TrainerSection**: `path: String`, `kind: String` (serialized as `type`), `loading_mode: TrainerLoadingMode` (enum: `SourceDirectory | CopyToPrefix`)
- **InjectionSection**: `dll_paths: Vec<String>`, `inject_on_launch: Vec<bool>`
- **SteamSection**: `enabled: bool`, `app_id: String`, `compatdata_path: String`, `proton_path: String`, `launcher: LauncherSection`
- **LauncherSection**: `icon_path: String`, `display_name: String`
- **RuntimeSection**: `prefix_path: String`, `proton_path: String`, `working_directory: String` — skipped from serialization when all fields empty
- **LaunchSection**: `method: String`, `optimizations: LaunchOptimizationsSection` — optimizations skipped when empty
- **LaunchOptimizationsSection**: `enabled_option_ids: Vec<String>` — skipped when empty

**All fields are `Clone`, `Default`, `Serialize`, `Deserialize`** — a duplicated profile is a byte-for-byte clone of the TOML content written to a new filename. No field transformations are needed during duplication.

### TOML File Format Example

```toml
[game]
name = "Elden Ring"
executable_path = "/games/elden-ring/eldenring.exe"

[trainer]
path = "/trainers/elden-ring.exe"
type = "fling"
loading_mode = "source_directory"

[injection]
dll_paths = ["/dlls/a.dll", "/dlls/b.dll"]
inject_on_launch = [true, false]

[steam]
enabled = true
app_id = "1245620"
compatdata_path = "/steam/compatdata/1245620"
proton_path = "/steam/proton/proton"

[steam.launcher]
icon_path = "/icons/elden-ring.png"
display_name = "Elden Ring"

[launch]
method = "steam_applaunch"
```

---

## Launcher Lifecycle

### Key Insight: Duplication Does NOT Trigger Launcher Operations

Launcher files (`.sh` scripts and `.desktop` entries) are only created via explicit export through `LauncherExport.tsx` -> `export_launchers` command. Duplication creates a new profile that has no associated launcher files on disk until the user explicitly exports.

### Profile Operations That Cascade to Launchers

| Operation                      | Launcher Cascade                                                          | Location                      |
| ------------------------------ | ------------------------------------------------------------------------- | ----------------------------- |
| `profile_delete`               | Best-effort cleanup of launcher files derived from profile's Steam fields | `commands/profile.rs:113-123` |
| `profile_rename`               | **None** — `fs::rename` on TOML only; no launcher file rename             | `toml_store.rs:150-165`       |
| `profile_save`                 | **None** — save does not touch launchers                                  | `toml_store.rs:93-98`         |
| `profile_duplicate` (proposed) | **None needed** — new profile has no exported launchers yet               | N/A                           |

### Launcher Path Derivation

Launcher slugs are derived from `display_name`, `steam_app_id`, and `trainer_path` — not from the profile name. So two profiles with identical game/trainer configuration would map to the same launcher slug. This is not a problem for duplication because:

1. Launchers are explicit exports, not automatic
2. The duplicate profile starts with no exported launchers
3. If the user later exports, the existing launcher check (`check_launcher_exists_for_request`) will detect the existing files and offer to overwrite

### Launcher File Locations

- Scripts: `~/.local/share/crosshook/launchers/{slug}-trainer.sh`
- Desktop entries: `~/.local/share/applications/crosshook-{slug}-trainer.desktop`

---

## Frontend Integration

### ProfileContext (context/ProfileContext.tsx)

- Wraps `useProfile()` hook and provides it via React context
- Adds derived values: `launchMethod`, `steamClientInstallPath`, `targetHomePath`
- Listens for `auto-load-profile` Tauri events at startup
- All profile components consume via `useProfileContext()`

### useProfile Hook (hooks/useProfile.ts)

Core state management for profiles. Key state and methods:

**State:**

- `profiles: string[]` — list of profile names from `profile_list`
- `selectedProfile: string` — currently loaded profile name
- `profileName: string` — editable name field (can differ from selectedProfile for new profiles)
- `profile: GameProfile` — current profile data
- `dirty: boolean` — tracks unsaved changes
- `loading / saving / deleting: boolean` — operation states
- `error: string | null`
- `profileExists: boolean` — derived: `profiles.includes(profileName.trim())`

**Methods relevant to duplication:**

- `hydrateProfile(name, profile)` — **This is the key method for duplication**. Sets profile name and data without loading from disk. Sets `selectedProfile` to the name only if it exists in `profiles`, otherwise `''`. Marks `dirty = true`. Used by community import and auto-populate flows.
- `selectProfile(name)` — Loads a profile from disk and selects it. Syncs metadata (last_used_profile, recent files).
- `persistProfileDraft(name, profile)` — Validates, normalizes, saves to disk, refreshes list, then loads the saved profile. Returns `{ ok: true }` or `{ ok: false, error }`.
- `refreshProfiles()` — Re-fetches the profile list from the backend.
- `saveProfile()` — Convenience wrapper: `persistProfileDraft(profileName, profile)`.

### Duplication Flow Options

**Option A: Backend-only duplicate (recommended)**

1. Frontend calls `invoke('profile_duplicate', { sourceName })` which returns the new name
2. Frontend calls `selectProfile(newName)` to load and display it
3. Profile list auto-refreshes on load

**Option B: Frontend-orchestrated duplicate**

1. Frontend calls `invoke('profile_load', { name: sourceName })` to get `GameProfile`
2. Frontend generates a unique name (e.g., `"My Profile (Copy)"`)
3. Frontend calls `hydrateProfile(newName, loadedProfile)` to populate the editor
4. User can optionally rename before saving
5. User clicks Save -> `persistProfileDraft()` writes to disk

**Option A is simpler** — the backend generates the unique name atomically (no race conditions) and the frontend just selects the result.

### ProfileActions Component (components/ProfileActions.tsx)

Simple button bar with Save and Delete actions. A "Duplicate" button would fit naturally here:

```tsx
<button
  type="button"
  className="crosshook-button crosshook-button--secondary"
  onClick={() => void onDuplicate()}
  disabled={!canDuplicate}
>
  Duplicate
</button>
```

`canDuplicate` should be `true` when `profileExists && !loading && !saving && !deleting`.

### ProfileFormSections Component (components/ProfileFormSections.tsx)

Contains the profile selector dropdown. The `profileSelector` prop provides:

- `profiles: string[]`
- `selectedProfile: string`
- `onSelectProfile: (name: string) => Promise<void>`

After duplication, the new profile should appear in this dropdown and be auto-selected.

### Normalization Pipeline

Profiles pass through normalization on both load and save:

- `normalizeProfileForEdit()` — Applied on load: resolves launch method, normalizes runtime section, strips launcher suffix from display_name, normalizes optimization IDs
- `normalizeProfileForSave()` — Applied on save: runs edit normalization + derives `game.name` and `launcher.display_name` from paths if empty

**For duplication**: The source profile is already saved/normalized. The duplicate just needs to be a clone written to a new filename. No re-normalization needed at the backend level — the frontend will normalize when it loads the duplicate.

---

## Configuration

### Paths

| Resource         | Path                                                           |
| ---------------- | -------------------------------------------------------------- |
| Profiles         | `~/.config/crosshook/profiles/{name}.toml`                     |
| Settings         | `~/.config/crosshook/settings.toml`                            |
| Recent files     | `~/.config/crosshook/recent.toml`                              |
| Launcher scripts | `~/.local/share/crosshook/launchers/{slug}-trainer.sh`         |
| Desktop entries  | `~/.local/share/applications/crosshook-{slug}-trainer.desktop` |

### Settings That Interact With Profiles

From `AppSettingsData` (settings/mod.rs:19-25):

- `auto_load_last_profile: bool` — If true, the app emits `auto-load-profile` event at startup
- `last_used_profile: String` — Updated on profile load via `syncProfileMetadata`

**Duplication does not need to update settings** — the duplicate is just a new profile. If the user selects it, `syncProfileMetadata` will update `last_used_profile` automatically.

---

## Relevant Files

| File                                                 | Role                                                                              |
| ---------------------------------------------------- | --------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/models.rs`        | `GameProfile` struct and all sub-section types                                    |
| `crates/crosshook-core/src/profile/toml_store.rs`    | `ProfileStore` with load/save/list/delete/rename + `validate_name`                |
| `crates/crosshook-core/src/profile/mod.rs`           | Public API re-exports for profile module                                          |
| `src-tauri/src/commands/profile.rs`                  | Tauri IPC command handlers for profile operations                                 |
| `src-tauri/src/lib.rs`                               | Command registration in `invoke_handler` macro                                    |
| `src/hooks/useProfile.ts`                            | Profile state management hook (hydrateProfile, persistProfileDraft)               |
| `src/context/ProfileContext.tsx`                     | React context wrapping useProfile + derived values                                |
| `src/components/ProfileActions.tsx`                  | Save/Delete button bar (add Duplicate button here)                                |
| `src/components/ProfileFormSections.tsx`             | Profile form with selector dropdown                                               |
| `src/types/profile.ts`                               | TypeScript `GameProfile` interface                                                |
| `crates/crosshook-core/src/export/launcher_store.rs` | Launcher lifecycle (not needed for duplication, but context for cascade behavior) |

---

## Gotchas and Edge Cases

1. **Name collision**: `list()` returns sorted names. A `duplicate` method must scan existing names to avoid overwriting. Pattern: `"{source} (Copy)"`, then `"{source} (Copy 2)"`, `"{source} (Copy 3)"`, etc.

2. **Name validation constraints**: Generated names must pass `validate_name()`. Parentheses, spaces, digits are all allowed. The only forbidden characters are `< > : " / \ | ? *`. Names like `"My Profile (Copy 2)"` are valid.

3. **Rename overwrites silently**: `ProfileStore::rename()` uses `fs::rename()` which overwrites the target if it exists. The duplicate name generator must check for existing files, not just rely on `list()`, because a non-`.toml` file at the target path wouldn't show up in `list()`.

4. **Profile name vs file name**: The profile name IS the file stem. There's no separate internal ID. So the duplicate's file stem is its identity.

5. **Launcher display_name is NOT the profile name**: `steam.launcher.display_name` is derived from the game name, not the profile name. A duplicated profile will have the same `display_name` as the source. This is correct — the launcher display name describes the game, not the profile.

6. **No concurrent write protection**: `ProfileStore` has no locking. The `save()` method is a simple `fs::write`. This is fine for duplication because we only write to a new (non-existent) path.

7. **RuntimeSection skip_serializing_if**: If the source profile has an empty `runtime` section, it won't be serialized in the TOML output. This is consistent behavior — the duplicate will also skip it, and `serde(default)` fills it in on load.

8. **Frontend state sync**: After backend duplication, the frontend must call `refreshProfiles()` or `selectProfile(newName)` to update the profiles list. `selectProfile` internally refreshes via `syncProfileMetadata`.
