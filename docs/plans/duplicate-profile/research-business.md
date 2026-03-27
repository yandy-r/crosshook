# Duplicate Profile -- Business Logic & Requirements Research

## Executive Summary

Profile duplication addresses a real user pain point: creating variant configurations for the same game (different trainers, optimization presets, or Proton versions) currently requires rebuilding profiles from scratch. The existing `ProfileStore` API already supports the core operation -- `load` + `save` under a new name -- making this a low-effort, high-value feature. The primary complexity lies in name conflict detection, launcher association semantics, and ensuring the duplicated profile is immediately usable.

## User Stories

### US-1: Variant Profiles for Optimization Testing

> As a user tuning Proton launch settings, I want to duplicate a working profile so I can experiment with different launch optimizations without risking my known-good configuration.

Context: The `LaunchOptimizationsSection` supports multiple option IDs (e.g., `disable_steam_input`, `use_gamemode`). Users need a safe way to A/B test combinations.

### US-2: Multiple Trainer Configurations

> As a user with multiple trainer executables for the same game, I want to clone my profile and swap the trainer path so I can quickly switch between FLiNG and WeMod setups.

Context: `TrainerSection` includes `path`, `kind`, and `loading_mode`. A duplicate-then-edit workflow is natural here.

### US-3: Community Profile Customization

> As a user who installed a community profile, I want to duplicate it before making local modifications so I can preserve the original community configuration for reference or re-sync.

Context: Community profiles are imported via `import_community_profile()` in `exchange.rs`. Once imported, they become regular TOML profiles with no special provenance tracking -- duplication treats them identically.

### US-4: Backup Before Risky Changes

> As a user about to make significant changes (switching launch method, changing Proton version), I want to clone my profile as a safety net.

## Business Rules

### Core Rules

1. **Source profile must exist**: The profile being duplicated must be a saved profile on disk (i.e., must pass `ProfileStore::load`).

2. **New name must be unique**: The generated name must not collide with any existing profile in the `ProfileStore::list()` result. The duplicate operation must never silently overwrite an existing profile. (Note: `rename` currently does allow overwrite -- `fs::rename` in `toml_store.rs:163` -- but duplicate must NOT follow this pattern.)

3. **New name must pass validation**: The generated name must satisfy `validate_name()` rules:
   - Not empty, not `.` or `..`
   - No absolute paths
   - No Windows reserved path characters: `< > : " / \ | ? *`
   - Trimmed of whitespace

4. **Full deep copy**: All profile fields are copied verbatim. The `GameProfile` struct derives `Clone`, so `profile.clone()` produces a complete deep copy of all sections: `game`, `trainer`, `injection`, `steam`, `runtime`, `launch` (including `optimizations`).

5. **No launcher association**: The duplicated profile does NOT inherit exported launchers (`.sh` scripts, `.desktop` entries). Launchers are derived from profile content at export time -- they are not stored within the profile itself. The duplicate starts with zero launcher files on disk.

6. **No runtime state inheritance**: The `runtime` section (prefix_path, proton_path, working_directory) is copied as-is. This is correct because runtime overrides are user-specified and part of the profile configuration, not ephemeral state.

### Name Generation Rules

7. **Auto-generated name pattern**: `"{original_name} (Copy)"`. If that collides, increment: `"{original_name} (Copy 2)"`, `"{original_name} (Copy 3)"`, etc.

8. **Collision detection**: Check against `ProfileStore::list()` which scans `.toml` files in the profiles directory. Profile names are case-sensitive (filesystem-dependent, but the store treats them as-is).

9. **Name length**: No explicit max length in `validate_name()`, but practical limits come from filesystem constraints (~255 bytes for filename). The `(Copy N)` suffix adds at most ~12 characters.

### Edge Cases

10. **Duplicating a profile that was itself a duplicate**: Works naturally. `"Game (Copy) (Copy)"` becomes the name, or `"Game (Copy) (Copy 2)"` if that collides. No special handling needed.

11. **Profile with empty/default fields**: Valid to duplicate. An incomplete profile (e.g., no trainer path) can still be saved. The `validateProfileForSave` function in `useProfile.ts` only requires `game.executable_path` -- but that validation is for the save-from-editor flow, not the duplicate flow. The backend `ProfileStore::save` has no field-level validation beyond name validation.

12. **Concurrent modifications**: Not a concern for the MVP. `ProfileStore` has no locking (noted in the `save_launch_optimizations` doc comment: "Concurrent save calls are not synchronized; the last completed write wins"). A duplicate is an atomic load-then-save-under-new-name.

## Workflows

### Primary Workflow: Duplicate via UI

```
1. User sees profile list (sidebar or dropdown) with a currently selected profile
2. User triggers "Duplicate" action (button in ProfileActions)
3. Frontend calls `invoke('profile_duplicate', { name: selectedProfile })`
4. Backend:
   a. Loads source profile via ProfileStore::load(name)
   b. Generates candidate name: "{name} (Copy)"
   c. Checks if candidate name exists via profile_path().exists()
   d. If collision, increments: "(Copy 2)", "(Copy 3)", ...
   e. Saves cloned profile via ProfileStore::save(new_name, &profile)
   f. Returns new_name to frontend
5. Frontend:
   a. Refreshes profile list
   b. Selects the newly created profile (loadProfile(new_name))
   c. Profile is now loaded in the editor, ready for modification
```

### Error Recovery

| Error                           | Behavior                                                               |
| ------------------------------- | ---------------------------------------------------------------------- |
| Source profile not found        | Return `ProfileStoreError::NotFound`; UI shows error                   |
| Invalid source name             | Return `ProfileStoreError::InvalidName`; UI shows error                |
| Generated name fails validation | Should not happen if source name is valid, but fall back to error      |
| Disk write failure              | Return `ProfileStoreError::Io`; no partial state (source is unchanged) |
| Name generation exhaustion      | Extremely unlikely; could cap at ~100 attempts and return error        |

### Alternative Workflow: Duplicate via CLI

The `crosshook-cli` crate could also expose duplication. This is out of scope for the initial implementation but the core logic lives in `crosshook-core` so CLI support is trivial to add later.

## Domain Model

### Profile Entity

```
GameProfile (models.rs)
  +-- game: GameSection { name, executable_path }
  +-- trainer: TrainerSection { path, kind, loading_mode }
  +-- injection: InjectionSection { dll_paths[], inject_on_launch[] }
  +-- steam: SteamSection { enabled, app_id, compatdata_path, proton_path, launcher: { icon_path, display_name } }
  +-- runtime: RuntimeSection { prefix_path, proton_path, working_directory }
  +-- launch: LaunchSection { method, optimizations: { enabled_option_ids[] } }
```

All fields derive `Clone`, `Serialize`, `Deserialize`, `PartialEq`, `Eq`, `Default`.

### Profile Identity

- **Primary key**: The profile name (= TOML filename without extension)
- **Storage**: `~/.config/crosshook/profiles/{name}.toml`
- **Uniqueness**: One file per name, enforced by filesystem

### Relationships

| Related Entity                   | Relationship to Profile                     | Duplication Impact                                                     |
| -------------------------------- | ------------------------------------------- | ---------------------------------------------------------------------- |
| Exported Launchers               | Derived from profile content at export time | NOT duplicated; new profile has no launchers until explicitly exported |
| Community Tap Origin             | No provenance tracking after import         | Duplicate is identical to duplicating any other profile                |
| App Settings (last_used_profile) | Points to a profile name                    | Updated when the duplicate is selected                                 |
| Recent Files                     | Aggregated from all profile saves           | Updated naturally when duplicate is saved/loaded                       |

### State Transitions

```
[No Profile] --save--> [Saved Profile] --duplicate--> [Saved Duplicate]
                                                            |
                                                            v
                                                    [User edits duplicate]
                                                            |
                                                            v
                                                    [Save modified duplicate]
```

The duplicate is immediately a fully saved profile -- there is no "draft" or "pending" state.

## Existing Codebase Integration

### Backend API Surface (ProfileStore -- toml_store.rs)

| Method         | Signature                                                     | Relevance to Duplicate                 |
| -------------- | ------------------------------------------------------------- | -------------------------------------- |
| `load`         | `(&self, name: &str) -> Result<GameProfile, E>`               | Read source profile                    |
| `save`         | `(&self, name: &str, profile: &GameProfile) -> Result<(), E>` | Write duplicate                        |
| `list`         | `(&self) -> Result<Vec<String>, E>`                           | Collision detection                    |
| `rename`       | `(&self, old: &str, new: &str) -> Result<(), E>`              | Pattern reference (rename overwrites!) |
| `profile_path` | `(&self, name: &str) -> Result<PathBuf, E>`                   | Check file existence                   |

Key observation: `rename` uses `fs::rename` which silently overwrites the target (see test `test_rename_overwrites_existing_target_profile` at line 405). The duplicate operation MUST NOT follow this pattern -- it must check for collisions first.

### Tauri Commands (profile.rs)

Current commands: `profile_list`, `profile_load`, `profile_save`, `profile_save_launch_optimizations`, `profile_delete`, `profile_rename`, `profile_import_legacy`.

A new `profile_duplicate` command fits naturally alongside these. It receives `State<'_, ProfileStore>` like all others.

### Frontend State (useProfile.ts)

Key functions for post-duplicate flow:

- `refreshProfiles()` -- reloads profile list from backend
- `selectProfile(name)` / `loadProfile(name)` -- loads and selects a profile
- `hydrateProfile(name, profile)` -- sets profile state without backend round-trip

The `persistProfileDraft` function shows the full save-then-reload pattern that the duplicate post-action should mirror.

### Frontend UI (ProfilesPage.tsx, ProfileActions.tsx)

`ProfileActions` currently renders Save and Delete buttons. The Duplicate button belongs here, next to Delete (or between Save and Delete). It should be:

- Disabled when no profile is selected (`!profileExists`)
- Disabled during save/delete/loading operations
- Enabled for any existing saved profile

## Success Criteria

1. Backend `profile_duplicate` command loads a profile, generates a unique "(Copy)" name, saves the clone, and returns the new name
2. Frontend Duplicate button appears in profile actions, is appropriately disabled/enabled
3. After duplication, the new profile is auto-selected in the editor
4. Name collisions produce incrementing suffixes, never overwrite
5. Error states surface clearly in the UI (same pattern as existing save/delete errors)
6. Existing tests pass; new tests cover the duplicate + collision logic

## Open Questions

1. **Should the `steam.launcher.display_name` be modified in the duplicate?** The launcher display name is user-facing and used for exported `.desktop` entries. Keeping the same display name could cause confusion if both profiles are exported. Recommendation: keep it as-is in the copy (user can edit), since launcher export is a separate explicit action.

2. **Should there be a limit on duplicate copies?** The auto-increment scheme could theoretically produce `(Copy 999)`. A reasonable cap (e.g., 100) prevents runaway situations, but this is an extreme edge case.

3. **Should the duplicate be a Tauri command or pure frontend logic?** The backend approach (single `profile_duplicate` command) is preferred because: (a) it keeps the name-generation and collision-detection logic server-side where the filesystem is, (b) it's atomic from the frontend's perspective, and (c) it's reusable by CLI. However, the frontend could alternatively do `profile_load` + name generation + `profile_save` -- the tradeoff is more IPC round-trips and a TOCTOU race on collision detection.
