# Feature Spec: Launcher Lifecycle Management

## Executive Summary

CrossHook currently exports launcher files (a `.sh` script and a `.desktop` entry) from profiles but provides no lifecycle management — deleting or renaming a profile leaves orphaned launcher files on disk, and there is no UI for managing existing launchers. This feature closes that gap by cascading profile lifecycle events to their associated launcher artifacts and adding manual launcher management controls to the Launcher Export panel. The implementation introduces a new `launcher_store` module in `crosshook-core` that discovers, deletes, and renames launchers by deriving file paths from launcher slugs using the existing deterministic `sanitize_launcher_slug()` chain. No new external dependencies are needed — `std::fs` and the existing `directories` crate cover all filesystem operations. The primary risks are slug collision (multiple profiles producing the same slug) and stale content in renamed files (both `.sh` and `.desktop` embed display names as plaintext). All competitive game launchers (Steam, Lutris, Heroic) attempt automatic cleanup but have documented bugs — CrossHook can differentiate by implementing robust lifecycle management with clear status indicators.

## External Dependencies

### APIs and Services

#### Freedesktop Desktop Entry Specification (v1.5)

- **Documentation**: <https://specifications.freedesktop.org/desktop-entry/latest-single/>
- **Practical guide**: <https://wiki.archlinux.org/title/Desktop_entries>
- **Key requirements**:
  - `Name=` field must be updated on rename (display name is plaintext)
  - `Exec=` field contains the absolute path to the shell script — MUST be updated on rename
  - `Comment=` field contains the display name — should be updated on rename
  - Files MUST be UTF-8 encoded
  - Implementations MUST NOT remove unknown fields during rewrites
  - When `Exec=` target is missing, compliant DEs auto-hide the entry (graceful degradation)
- **CrossHook convention**: `crosshook-{slug}-trainer.desktop` in `$XDG_DATA_HOME/applications/`

#### XDG Base Directory Specification

- **Documentation**: <https://specifications.freedesktop.org/basedir/latest/>
- **Key variable**: `XDG_DATA_HOME` (default: `$HOME/.local/share`)
- **Note**: Current `export_launchers()` hardcodes `~/.local/share/` instead of using `BaseDirs::data_dir()` — flagged as a refactoring candidate during this feature

#### Desktop Cache Invalidation

- Desktop environments (GNOME, KDE) use inotify to auto-detect `.desktop` file changes
- `update-desktop-database` is optional (CrossHook entries don't declare `MimeType=`)
- Steam Deck Gaming Mode does not use desktop menus — N/A

### Libraries and SDKs

| Library       | Version       | Purpose                                           | Installation          |
| ------------- | ------------- | ------------------------------------------------- | --------------------- |
| `std::fs`     | stdlib        | All file operations (delete, rename, read, write) | Built-in              |
| `directories` | v5 (existing) | XDG path resolution via `BaseDirs::data_dir()`    | Already in Cargo.toml |

**Evaluated but NOT recommended**: `freedesktop-desktop-entry` (MPL-2.0 license, read-only focus), `deentry` (stale, low maturity), `xdg` (redundant with `directories`), `notify` (filesystem watching unnecessary — CrossHook is sole manager)

### External Documentation

- [Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry/latest-single/): File format, required fields, naming rules
- [XDG Base Directory Spec](https://specifications.freedesktop.org/basedir/latest/): Standard paths for user data
- [ArchWiki: Desktop Entries](https://wiki.archlinux.org/title/Desktop_entries): Practical Linux desktop entry guide

## Business Requirements

### User Stories

**Primary User: Game Player / Steam Deck User**

- As a player who deletes a profile, I want the associated launcher script and desktop entry cleaned up automatically so my application menu and launcher directory do not accumulate stale shortcuts
- As a player who renames a profile, I want the associated launcher files renamed and their internal display name updated so shortcuts remain accurate and findable
- As a player managing multiple game profiles, I want to see which launchers exist for the current profile and be able to delete or rename them manually in case automatic cleanup missed something
- As a Steam Deck user, I want orphaned `.desktop` entries removed so my Game Mode library stays clean and only shows launchers for games I actually have configured

### Business Rules

1. **Profile Deletion Cascades to Launcher Cleanup**
   - When a profile is deleted, the system must check whether launcher artifacts exist and delete them
   - Validation: Derive expected launcher paths from profile data before deletion; verify they exist on disk
   - Exception: If the profile was never exported (no launcher exists), deletion proceeds without launcher cleanup — not an error

2. **Profile Rename Cascades to Launcher Rename**
   - When a profile is saved under a new name, the system must rename both launcher files and update internal content (`Name=`, `Exec=`, `Comment=` in `.desktop`; comment header in `.sh`)
   - Validation: Old launcher files must exist before attempting rename; new slug must not collide with a different profile's launcher
   - Exception: If no launcher exists under the old name, the rename proceeds silently

3. **Launcher Path Derivation Must Be Deterministic**
   - Derivation chain: `launcher_name` → `sanitize_launcher_slug()` → file paths
   - Script: `$XDG_DATA_HOME/crosshook/launchers/{slug}-trainer.sh`
   - Desktop entry: `$XDG_DATA_HOME/applications/crosshook-{slug}-trainer.desktop`

4. **Manual Launcher Management Is Always Available**
   - The Launcher Export panel must expose delete and rename actions for any launcher associated with the current profile
   - System should check launcher file existence on disk before showing management actions

5. **Best-Effort Cascade — Profile Operations Never Blocked**
   - Profile deletion succeeds even if launcher cleanup fails
   - Launcher cleanup errors are surfaced as warnings, not blocking errors

6. **Native-Method Profiles Skip Launcher Lifecycle**
   - Launcher export only supports `steam_applaunch` and `proton_run`
   - Profiles with `native` launch method never have launchers — lifecycle events skip cleanup

### Edge Cases

| Scenario                                                                   | Expected Behavior                                                                 | Notes                                              |
| -------------------------------------------------------------------------- | --------------------------------------------------------------------------------- | -------------------------------------------------- |
| Profile deleted, launcher script manually moved                            | System attempts delete at canonical paths; `NotFound` is a no-op                  | Cannot track manually relocated files              |
| Profile renamed but launcher exported with custom `launcher_name` override | System derives slug from the same `launcher_name` fallback chain                  | `resolve_display_name()` must be reused            |
| Two profiles export launchers with same slug                               | Warn but don't prevent; deleting one removes both                                 | Inherent limitation of lossy slug derivation       |
| Launcher exported, then profile fields changed without re-export           | System uses current profile data for path derivation; may not match on-disk files | "Stale" status indicator helps user awareness      |
| Permission denied on launcher files                                        | Report error to user; do not block profile operation                              | Script: `0o755`, desktop entry: `0o644`            |
| Slug unchanged after rename (display name changed but slug is identical)   | Rewrite file content in place with updated display name                           | `Name=` and `Comment=` change even if slug doesn't |

### Success Criteria

- [ ] Deleting a profile automatically removes associated launcher files when they exist
- [ ] Deleting a profile succeeds even when no launcher was ever exported
- [ ] Renaming a profile updates associated launcher files (paths and internal content) when they exist
- [ ] The Launcher Export panel shows delete/rename actions when a launcher exists for the current profile
- [ ] Manual launcher deletion requires user confirmation
- [ ] Launcher operations report clear error messages on failure
- [ ] Native-method profiles skip launcher lifecycle operations entirely
- [ ] Existing Rust tests pass and new tests cover delete/rename logic
- [ ] Launcher slug derivation is consistent between export, delete, and rename operations

## Technical Specifications

### Architecture Overview

```
ProfileEditor.tsx                  LauncherExport.tsx
  |                                    |
  | profile_delete / profile_rename   | export_launchers / delete_launcher
  v                                    v
commands/profile.rs  <---------->  commands/export.rs
  |                                    |
  | cascades via                       | uses
  v                                    v
export::launcher_store  <------  export::launcher (existing)
  |
  | fs operations: check, delete, rename, list
  v
~/.local/share/crosshook/launchers/{slug}-trainer.sh
~/.local/share/applications/crosshook-{slug}-trainer.desktop
```

### Data Models

#### LauncherInfo (new)

| Field                | Type   | Constraints | Description                              |
| -------------------- | ------ | ----------- | ---------------------------------------- |
| display_name         | String | Required    | Display name extracted from `Name=` line |
| launcher_slug        | String | Required    | Slug derived from display name           |
| script_path          | String | Required    | Absolute path to `.sh` script            |
| desktop_entry_path   | String | Required    | Absolute path to `.desktop` entry        |
| script_exists        | bool   | Required    | Whether the script file exists on disk   |
| desktop_entry_exists | bool   | Required    | Whether the desktop entry exists on disk |

#### LauncherDeleteResult (new)

| Field                 | Type   | Description                           |
| --------------------- | ------ | ------------------------------------- |
| script_deleted        | bool   | Whether the script was deleted        |
| desktop_entry_deleted | bool   | Whether the desktop entry was deleted |
| script_path           | String | Path that was targeted                |
| desktop_entry_path    | String | Path that was targeted                |

#### LauncherRenameResult (new)

| Field                  | Type   | Description                           |
| ---------------------- | ------ | ------------------------------------- |
| old_slug               | String | Previous launcher slug                |
| new_slug               | String | New launcher slug                     |
| new_script_path        | String | New script file path                  |
| new_desktop_entry_path | String | New desktop entry path                |
| script_renamed         | bool   | Whether the script was renamed        |
| desktop_entry_renamed  | bool   | Whether the desktop entry was renamed |

#### File Path Convention

Given a launcher slug (e.g., `elden-ring-deluxe`) and a resolved home path:

| File          | Path Pattern                                                        |
| ------------- | ------------------------------------------------------------------- |
| Shell script  | `{home}/.local/share/crosshook/launchers/{slug}-trainer.sh`         |
| Desktop entry | `{home}/.local/share/applications/crosshook-{slug}-trainer.desktop` |

Slug derivation chain (from `launcher.rs`):

1. `resolve_display_name(launcher_name, steam_app_id, trainer_path)` picks the first non-empty value
2. `sanitize_launcher_slug(display_name)` lowercases, replaces non-alphanumeric runs with `-`, trims

### API Design (Tauri IPC)

#### New Commands

##### `check_launcher_exists`

**Purpose**: Check whether launcher files exist for a given profile's launcher slug
**Request**: `{ launcher_slug: string, target_home_path: string }`
**Response**: `LauncherInfo`

##### `delete_launcher`

**Purpose**: Delete the `.sh` script and `.desktop` entry for a given launcher slug
**Request**: `{ launcher_slug: string, target_home_path: string }`
**Response**: `LauncherDeleteResult`

##### `rename_launcher`

**Purpose**: Rename launcher files from one slug to another, updating internal content
**Request**: `{ old_launcher_slug: string, new_display_name: string, new_launcher_icon_path: string, target_home_path: string }`
**Response**: `LauncherRenameResult`
**Strategy**: Write new files with correct content, then delete old files (not in-place rename, because `.sh` and `.desktop` embed display names as plaintext)

##### `list_launchers`

**Purpose**: Scan the launchers directory for all CrossHook-generated launcher scripts
**Request**: `{ target_home_path: string }`
**Response**: `Vec<LauncherInfo>`
**Discovery**: Read `*-trainer.sh` files, derive slug, check for matching `.desktop`, extract `Name=`

##### `profile_rename`

**Purpose**: Rename a profile and cascade launcher updates
**Request**: `{ old_name: string, new_name: string }`
**Response**: `()`

#### Modified Commands

- **`profile_delete`**: Load profile data before deletion to derive launcher slug → best-effort delete launcher files → delete profile TOML
- **`profile_save`**: Frontend detects rename (old name differs from new name) and calls `profile_rename` instead

### System Integration

#### Files to Create

- `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`: New module with `check_launcher_exists`, `delete_launcher_files`, `rename_launcher_files`, `list_launchers`, and all new types (`LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`)

#### Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/export/mod.rs`: Add `pub mod launcher_store;` and re-export new public types
- `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`: Elevate `resolve_display_name`, `combine_host_unix_path`, `build_desktop_entry_content`, `write_host_text_file`, `build_trainer_script_content` from private to `pub(crate)` visibility
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: Add `ProfileStore::rename(old_name, new_name)` method using atomic `fs::rename`
- `src/crosshook-native/src-tauri/src/commands/export.rs`: Add `check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers` Tauri commands
- `src/crosshook-native/src-tauri/src/commands/profile.rs`: Add `profile_rename` Tauri command; modify `profile_delete` to cascade launcher deletion
- `src/crosshook-native/src-tauri/src/lib.rs`: Register new Tauri commands in `invoke_handler`
- `src/crosshook-native/src/components/LauncherExport.tsx`: Add launcher status indicator, "Delete Launcher" and "Rename Launcher" buttons, and launcher existence checking on mount
- `src/crosshook-native/src/hooks/useProfile.ts`: Detect rename scenario (`profileName !== selectedProfile`), invoke `profile_rename` instead of save+delete
- `src/crosshook-native/src/types/profile.ts` or new `src/crosshook-native/src/types/launcher.ts`: Add TypeScript `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult` interfaces

#### Configuration

- No new configuration keys required
- Future: `auto_delete_launchers` setting for power users who want automatic cleanup without confirmation

## UX Considerations

### User Workflows

#### Primary Workflow: Profile Delete with Launcher Cleanup

1. **User clicks Delete** in ProfileEditor
   - System: Loads profile data to derive launcher slug
2. **System checks for launchers**
   - System: Scans canonical paths for script and `.desktop` entry
3. **Confirmation dialog** (if launchers exist)
   - User sees: "Delete profile **[Name]** and its launcher files: `[slug]-trainer.sh`, `crosshook-[slug]-trainer.desktop`?"
   - Buttons: "Delete Profile and Launcher" (red) | "Cancel" (default focus)
4. **Execution**
   - System: Deletes profile TOML, then best-effort deletes launcher files
5. **Feedback**
   - Success: Toast "Profile and launcher files deleted"
   - Partial failure: Warning banner with failed file path and "Retry" action

#### Primary Workflow: Profile Rename with Launcher Update

1. **User changes profile name** and saves
   - System: Detects rename (old name differs from new name)
2. **Inline notification** (if launchers exist, not a modal)
   - User sees: "Renaming will update launcher files. Old: [old paths] → New: [new paths]"
   - Buttons: "Save and Update Launcher" (primary) | "Save Without Updating" | "Cancel"
3. **Execution** (write-then-delete)
   - System: Save new profile, regenerate launcher files with new display name/paths, delete old files
4. **Feedback**
   - Success: "Profile renamed. Launcher files updated."

#### Manual Launcher Management

1. **Status display**: Launcher Export panel shows "Exported" (green) / "Not Exported" (gray) / "Stale" (amber) status badge
2. **Delete**: "Delete Launcher" button with inline confirmation (click-again pattern, 3-second timeout)
3. **Update**: "Update Launcher" button visible when status is "Stale" (non-destructive overwrite)

#### Error Recovery Workflow

1. **Error Occurs**: Permission denied or partial failure
2. **User Sees**: Error banner with specific file path and error message
3. **Recovery**: "Retry" button or manual deletion instructions

### UI Patterns

| Component                   | Pattern                         | Notes                                                                |
| --------------------------- | ------------------------------- | -------------------------------------------------------------------- |
| Profile delete confirmation | Modal dialog with file list     | Custom React modal (Tauri native dialog lacks custom content)        |
| Rename cascade notification | Inline notification panel       | Between name field and Save button — avoids breaking edit flow       |
| Manual launcher delete      | Inline click-again confirmation | 3-second timeout, reverts on blur                                    |
| Launcher status             | Colored dot + text label        | Green/gray/amber; two differentiating attributes for WCAG            |
| Destructive buttons         | Red color scheme                | `rgba(185, 28, 28, 0.16)` bg with `rgba(248, 113, 113, 0.28)` border |

### Accessibility Requirements

- Gamepad: A=confirm, B=cancel mapping via existing `useGamepadNav` hook
- Focus trap in confirmation dialogs; default focus on "Cancel" (safe action)
- Minimum 44px touch targets (already met in codebase)
- No hover-dependent interactions (Steam Deck has no hover state)
- Status indicators use color + text label (not color alone)

### Performance UX

- **Loading States**: File operations are near-instantaneous (<1ms for local files) — no spinner needed, just disable button and show "Deleting..."/"Updating..." label
- **Optimistic Updates**: Immediately update launcher status in UI; revert on backend failure
- **Error Feedback**: Synchronous operations, so errors appear immediately

## Recommendations

### Implementation Approach

**Recommended Strategy**: Phased implementation starting with stateless slug derivation for the core MVP, with the API designed so a manifest registry can be added later for orphan detection and stale tracking.

**Key design decision — Stateless vs. Manifest**:

| Approach                                | Pros                                                                | Cons                                                                                    |
| --------------------------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| **Stateless (derive paths from slugs)** | Simpler, no state drift, no migration, works for CLI too            | Cannot detect orphans from deleted profiles, lossy slug makes reverse lookup impossible |
| **Manifest registry (TOML file)**       | Single source of truth, orphan detection, survives profile deletion | New state to manage, can become stale on crash, adds complexity                         |

**Recommendation**: Start with stateless derivation (Phase 1-2). The deterministic slug mapping is sufficient for the core delete/rename cascades. Add a lightweight manifest in Phase 3 if orphan detection proves necessary. The `# Generated by CrossHook` watermark in launcher files provides a safety check regardless of approach.

**Phasing:**

1. **Phase 1 - Foundation**: Launcher delete core + cascade on profile delete + launcher status indicator
2. **Phase 2 - Rename + Manual Management**: Profile rename + launcher rename cascade + manual delete/rename UI in Launcher Export panel
3. **Phase 3 - Polish**: Orphan detection, stale launcher warnings, bulk management, optional manifest registry

### Technology Decisions

| Decision             | Recommendation                                                     | Rationale                                                                |
| -------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| Tracking approach    | Stateless slug derivation (Phase 1-2)                              | Simpler; no migration; deterministic paths sufficient for core features  |
| Cascade behavior     | Best-effort with UI notification                                   | Profile operations should never be blocked by launcher filesystem issues |
| Rename strategy      | Write-then-delete (not in-place rename)                            | `.sh` and `.desktop` embed display names; content must be regenerated    |
| Home path resolution | Backend-resolved for cascades, frontend-provided for manual ops    | Consistent with existing `export_launchers` pattern                      |
| Profile rename       | Atomic `fs::rename` via new `ProfileStore::rename` method          | Avoids window where two copies exist or delete fails leaving duplicate   |
| Confirmation UX      | Modal for profile+launcher delete; inline for launcher-only delete | Tiered severity matches action impact                                    |

### Quick Wins

- **Cascade delete on `profile_delete`**: Even before a full UI, computing expected launcher paths from profile data and attempting `fs::remove_file` during `profile_delete` provides immediate value
- **Launcher existence check**: Add `check_launcher_exists` command and show status badge in Launcher Export panel — minimal UI change, high visibility
- **`X-CrossHook-Profile` metadata**: Add an `X-CrossHook-Profile={profile_name}` line to exported `.desktop` files for future orphan detection

### Future Enhancements

- Auto-re-export on profile save when launcher-relevant fields change
- Bulk launcher management panel in Settings
- Launcher sync status badge in profile selector dropdown
- Orphan scanner on app startup
- Manifest registry for comprehensive tracking (Phase 3)
- CLI subcommands: `crosshook launcher list`, `crosshook launcher delete`, `crosshook launcher rename`
- macOS support (`.app` bundles or `launchd` integration instead of `.desktop`)

## Risk Assessment

### Technical Risks

| Risk                                                              | Likelihood | Impact | Mitigation                                                                                                |
| ----------------------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------------- |
| Deleting wrong file (user-created `.desktop` with similar naming) | Low        | High   | Verify `# Generated by CrossHook` watermark before deletion; validate path is within expected directories |
| Slug collision (two profiles produce same slug)                   | Low        | Medium | Document as known limitation; UI warning when detected; manifest tracks exact owner (Phase 3)             |
| Filesystem permissions prevent deletion                           | Low        | Medium | Surface clear error messages with exact path; never block profile operations                              |
| Profile renamed outside CrossHook (manual TOML rename)            | Medium     | Low    | Orphan detection (Phase 3); best-effort cleanup using current profile data                                |
| XDG_DATA_HOME override causes path mismatch                       | Low        | Medium | Use `BaseDirs::data_dir()` in new code; refactor export to use it too                                     |
| Symlink attack: launcher file is a symlink                        | Very Low   | High   | Verify target is a regular file before deletion                                                           |

### Integration Challenges

- **`profile_delete` needs launcher awareness without tight coupling**: Handle cleanup at the Tauri command level (after store delete), not inside `ProfileStore` itself
- **Frontend state synchronization**: Re-query launcher status after any mutation via `check_launcher_exists`
- **Rename detection in `useProfile`**: Must compare old/new profile name to detect rename vs. create-new scenarios

### Security Considerations

- **Path traversal**: `sanitize_launcher_slug` strips path separators, but any code accepting user paths for deletion must validate targets are within expected directories and match the `crosshook-` prefix
- **Symlink following**: Verify target is a regular file (not a symlink) before deletion
- **Watermark verification**: Always check for `# Generated by CrossHook` comment before deleting any file

## Task Breakdown Preview

### Phase 1: Foundation

**Focus**: Cascade delete on profile deletion + launcher status indicator

**Tasks**:

- Create `export/launcher_store.rs` with `LauncherInfo`, `LauncherDeleteResult` types and `check_launcher_exists`, `delete_launcher_files` functions
- Elevate private functions in `launcher.rs` to `pub(crate)` visibility
- Add `check_launcher_exists` and `delete_launcher` Tauri commands
- Modify `profile_delete` to cascade launcher deletion (best-effort)
- Add launcher status indicator to `LauncherExport.tsx`
- Add TypeScript types for `LauncherInfo` and `LauncherDeleteResult`
- Write Rust unit tests for delete logic (tempdir fixtures)

**Parallelization**: Core Rust module + TypeScript types can proceed in parallel; Tauri commands depend on both

### Phase 2: Rename + Manual Management

**Focus**: Profile rename cascade + manual launcher management UI

**Dependencies**: Phase 1 must complete (launcher_store module exists)

**Tasks**:

- Add `ProfileStore::rename` method
- Add `rename_launcher_files` and `list_launchers` functions to `launcher_store`
- Add `profile_rename`, `rename_launcher`, `list_launchers` Tauri commands
- Add rename detection to `useProfile.ts` (compare old/new profile name)
- Add manual delete/rename buttons to `LauncherExport.tsx` with confirmation UX
- Add inline rename notification panel
- Add confirmation modal for profile-delete-with-launcher cascade
- Write Rust unit tests for rename logic

**Parallelization**: Profile rename (core + command) and manual management UI can proceed in parallel; launcher rename depends on both

### Phase 3: Polish

**Focus**: Stale detection, orphan management, robustness

**Tasks**:

- Add "Stale" status detection (compare profile state vs. launcher content)
- Add `X-CrossHook-Profile` metadata to exported `.desktop` files
- Optional: Introduce launcher manifest for orphan tracking
- Add orphan scanner and cleanup UI in Settings panel
- Add file watermark verification before all delete operations
- Symlink safety checks

**Parallelization**: Stale detection and orphan management are independent

### Estimated Complexity

- **Total tasks**: ~18-22 discrete implementation tasks across 3 phases
- **Critical path**: `launcher_store` module → Tauri commands → cascade integration → frontend UI
- **Phase 1**: Core scope, establishes the foundation
- **Phase 2**: Most complex (rename involves content regeneration + file moves + UI)
- **Phase 3**: Lower risk, primarily read operations and UI polish

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **Manifest vs. Stateless**
   - Options: (A) Stateless derivation only, (B) Manifest registry from day one, (C) Phased — stateless now, manifest later
   - Impact: Manifest enables orphan detection and stale tracking; stateless is simpler
   - Recommendation: (C) Phased approach

2. **Explicit Rename vs. Implicit Detection**
   - Options: (A) Add explicit `profile_rename` Tauri command, (B) Detect rename from old/new name comparison in `profile_save`
   - Impact: Explicit rename is cleaner but requires new command + UI flow; implicit detection is lower-effort but fragile
   - Recommendation: (A) Explicit `profile_rename` command

3. **Confirmation UX for Automatic Cascade**
   - Options: (A) No extra confirmation — user already confirmed profile delete, (B) Enhanced confirmation dialog listing affected launcher files
   - Impact: (B) is safer and more transparent; (A) avoids confirmation fatigue
   - Recommendation: (B) Enhanced dialog when launchers exist, simple dialog when they don't

4. **Rename Cascade Behavior**
   - Options: (A) Automatic — always rename launchers when profile is renamed, (B) Opt-in — show inline notification and let user choose
   - Impact: Automatic is simpler but may surprise users; opt-in gives control
   - Recommendation: (B) Opt-in with clear explanation

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Freedesktop .desktop spec, XDG paths, Rust crate evaluation, file operation patterns
- [research-business.md](./research-business.md): User stories, business rules, workflows, domain model, codebase integration analysis
- [research-technical.md](./research-technical.md): Architecture design, data models, Tauri IPC contracts, system constraints, codebase changes
- [research-ux.md](./research-ux.md): UX patterns, competitive analysis (Steam/Lutris/Heroic), gamepad accessibility, confirmation dialogs
- [research-recommendations.md](./research-recommendations.md): Implementation strategy, risk assessment, alternative approaches, task breakdown
