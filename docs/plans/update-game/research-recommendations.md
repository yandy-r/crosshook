# Recommendations: update-game

## Executive Summary

The update-game feature should be modeled after the existing Install Game infrastructure -- reusing the same `build_install_command` pattern (Proton prefix environment, runtime helpers, log attachment) but with a simpler flow since the prefix already exists and no candidate discovery is needed. The strongest approach is to extend the Install Game page with a dedicated "Update" tab or section that loads from a selected profile, inheriting its prefix and Proton path, then runs the update executable. Key risks are prefix corruption from failed updates and wrong-prefix targeting; both are mitigated by requiring profile selection (which pins the prefix) and an optional pre-update backup.

## Implementation Recommendations

### Recommended Approach

Build update-game as a **sibling to install-game** inside the same `install` module, reusing most of the same Rust primitives. The core flow is:

1. User selects an existing profile (which provides prefix path, Proton path, and display name).
2. User chooses an update executable (.exe) via file browser.
3. Backend validates: profile exists, prefix directory exists, update executable exists, Proton path is executable.
4. Backend runs the update executable through the same `new_direct_proton_command` + `apply_runtime_proton_environment` + `attach_log_stdio` pipeline used by `build_install_command` in `install/service.rs`.
5. Log output is streamed to the frontend via Tauri events (same pattern as `launch/` commands).
6. On completion, a summary result is returned (success/failure, log path, exit code).

This approach directly replaces the manual script workflow because it automates exactly what the user's script does: set `WINEPREFIX` / `STEAM_COMPAT_DATA_PATH` to the correct prefix, then invoke `proton run <update.exe>`.

### Technology Choices

| Component         | Recommendation                                                                                                     | Rationale                                                                                                                                                         |
| ----------------- | ------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Backend module    | New `update` module in `crosshook-core`, sibling to `install`                                                      | Keeps concerns separated from install (which has candidate discovery, prefix provisioning, profile generation); update is a simpler "run exe in prefix" operation |
| Command builder   | Reuse `new_direct_proton_command` + `apply_runtime_proton_environment` from `launch/runtime_helpers.rs`            | Already tested, handles `pfx` vs standalone prefix resolution, applies correct env vars                                                                           |
| Log streaming     | Reuse `spawn_log_stream` pattern from `src-tauri/src/commands/launch.rs`                                           | Users expect real-time log output; the launch module already solves this with the `launch-log` Tauri event                                                        |
| Frontend state    | New `useUpdateGame` hook modeled after `useInstallGame` but simpler (no candidate discovery, no prefix resolution) | Consistent pattern; the install hook is 580 lines, update should be approximately 200                                                                             |
| Prefix resolution | Read directly from the selected profile's `runtime.prefix_path` or `steam.compatdata_path`                         | No guessing needed; the profile already knows its prefix                                                                                                          |
| Tauri command     | New `update_game` and `validate_update_request` commands in `src-tauri/src/commands/update.rs`                     | Follows the pattern of `install.rs` exactly                                                                                                                       |

### Phasing Strategy

1. **Phase 1 - MVP**: Core update execution
   - `UpdateGameRequest` / `UpdateGameResult` models in `crosshook-core/src/update/models.rs`
   - `validate_update_request` and `update_game` functions in `crosshook-core/src/update/service.rs`
   - Tauri commands `update_game` and `validate_update_request` in `src-tauri/src/commands/update.rs`
   - `useUpdateGame` hook with basic state management
   - UI section on the Install page (or new "Update" tab) with profile selector, update exe browser, Proton selector, and run button
   - Log streaming to console drawer via existing `launch-log` event pattern

2. **Phase 2 - Enhancement**: Safety and discoverability
   - Pre-update prefix backup (tar/rsync snapshot of `drive_c` before running the updater)
   - Update history log (append-only TOML or JSON recording profile name, update exe, timestamp, exit code, log path)
   - Auto-detect update executables from a configurable directory (e.g., `~/Downloads/*.exe` or a profile-specific updates folder)
   - Sidebar route promotion from Install page sub-tab to first-class "Update Game" route (if usage justifies it)

3. **Phase 3 - Polish**: Batch and community integration
   - Batch updates: queue multiple profile/update-exe pairs and run sequentially
   - Community tap integration: tap manifests could include `update_patches` entries with URLs/checksums
   - Update diff report: compare prefix contents before and after to show what changed
   - Rollback from backup

### Quick Wins

- **Profile-aware prefix resolution**: Since profiles already store `runtime.prefix_path` or `steam.compatdata_path`, the update flow can skip all prefix discovery logic and read directly from the profile. This eliminates the most error-prone part of the manual workflow.
- **Reuse `resolve_wine_prefix_path`**: The helper in `runtime_helpers.rs` (line 94-105) already handles the `pfx` subdirectory heuristic, so both `compatdata`-style and standalone prefixes work without special casing.
- **Log path convention**: Use `update-<profile-slug>-<timestamp>.log` under `/tmp/crosshook-logs/`, exactly like install and launch logs, so the console drawer picks them up automatically.

## Improvement Ideas

### Related Features

- **Prefix Backup Before Update (High Value)**: Create a compressed snapshot of the prefix's `drive_c` before running the updater. The install module's `provision_prefix` function shows the pattern for prefix path manipulation. A simple `tar czf` of `drive_c` to `~/.local/share/crosshook/backups/<profile>-<timestamp>.tar.gz` would provide rollback capability. This directly addresses the #1 risk (prefix corruption).
- **Update History Log**: Append each update operation to `~/.config/crosshook/update-history.toml` with fields: profile name, update exe path, timestamp, exit code, log path, prefix backup path. This gives users an audit trail and enables future "repeat last update" functionality.
- **Auto-Detect Update Executables**: Scan a configurable directory (default: `~/Downloads`) for `.exe` files matching known patterns (e.g., containing "update", "patch", "setup") and present them as suggestions. This could reuse the scoring/ranking approach from `install/discovery.rs` adapted for file names.
- **Integration with Community Taps**: The community profile schema (`profile/community_schema.rs`) could be extended with an `updates` section listing known update patches (URL, checksum, compatible versions). The community browser would then surface available updates per installed profile.

### Future Enhancements

- **Batch Updates**: Allow selecting multiple profiles and their respective update executables, then run them sequentially with per-profile logging. Complexity: medium (UI queue management, sequential async execution).
- **Working Directory Override**: Allow specifying a custom working directory for the update executable (some installers need to run from their own directory). The existing `apply_working_directory` helper supports this already.
- **Post-Update Verification**: After the update executable completes, re-run the candidate discovery scanner from `install/discovery.rs` to verify the game executable still exists in the prefix. Complexity: low (just call `discover_game_executable_candidates`).
- **Update Notifications**: If a profile has a known community tap, check for new update entries on app startup and notify the user via Tauri events. Complexity: medium-high.

## Risk Assessment

### Technical Risks

| Risk                                                                     | Likelihood | Impact   | Mitigation                                                                                                                                                                                                    |
| ------------------------------------------------------------------------ | ---------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Prefix corruption from failed/interrupted update                         | Medium     | High     | Phase 2 prefix backup; always log the full update session; consider journaling the update operation                                                                                                           |
| Wrong prefix targeted (data loss for another game)                       | Low        | Critical | Require profile selection (never free-text prefix); validate prefix path exists and contains `drive_c`; display profile name + prefix path prominently for user confirmation before running                   |
| Proton version mismatch (update exe needs different Proton than profile) | Medium     | Medium   | Default to the profile's Proton version but allow override via a Proton selector (reuse the `ProtonPathField` component from `InstallGamePanel.tsx`); warn if the selected version differs from the profile's |
| File permission issues (prefix owned by different user/permissions)      | Low        | Medium   | Validate prefix is writable before starting; existing `provision_prefix` pattern checks `metadata.is_dir()` -- extend with write permission check                                                             |
| Security: running arbitrary executables via Proton                       | Medium     | Medium   | All executables already run in a Proton sandbox (not native); the attack surface is the same as Install Game; display the full path for user review; never auto-execute without user confirmation             |
| Update executable expects interactive input (GUI installer)              | High       | Low      | This is expected and handled; Proton will display the GUI window just like install does; the log streaming captures stdout/stderr while the GUI runs                                                          |
| Large prefix backup takes too long                                       | Medium     | Low      | Make backup optional (Phase 2); use incremental backup (rsync) for large prefixes; show progress indicator                                                                                                    |
| Race condition: user launches game while update is running               | Low        | Medium   | Disable the Launch button for the selected profile while an update is in progress; use Tauri state to track active update operations                                                                          |

### Integration Challenges

- **Tauri command registration**: Adding new commands requires updating the `invoke_handler` array in `src-tauri/src/lib.rs` (currently 32 commands). The pattern is mechanical but easy to miss.
- **Frontend routing**: If update gets its own sidebar route (rather than an Install page sub-section), the `AppRoute` type union in `Sidebar.tsx`, `VALID_APP_ROUTES` in `App.tsx`, and `ContentArea.tsx` switch must all be updated simultaneously. The `ContentArea` has an exhaustive switch (`never` default), so a missing case is caught at compile time.
- **Event channel reuse**: The update log streaming should use the same `launch-log` event name (or a new `update-log` event) -- using the same name means the existing `ConsoleDrawer` picks it up automatically, but it could mix update and launch logs if both are active. A dedicated `update-log` event is cleaner.
- **Profile store dependency**: The update flow needs to load the profile to read its prefix/Proton paths. The existing `profile_load` Tauri command already does this. The frontend should call `profile_load` to populate the update form, or the backend `update_game` command should accept a profile name and load it internally.

## Alternative Approaches

### Option A: Extend Install Game Page with an "Update" Tab/Section

- **Pros**:
  - Keeps related Proton-prefix operations together ("Setup" section in sidebar already houses Install)
  - Shares Proton install selector, log display, and prefix path display components
  - Minimal routing/navigation changes (no new sidebar item, no new `AppRoute` variant)
  - Lower design overhead; the Install page layout (sections, status card, action buttons) translates directly
  - The Install page's `ProfileReviewModal` pattern is not needed for updates (no profile generation), simplifying the page
- **Cons**:
  - The Install page is already complex (~450 lines in `InstallPage.tsx`); adding update state makes it larger
  - Conceptual mismatch: "Install Game" is a setup-time action; "Update Game" is a maintenance action on an existing profile
  - Users must navigate to "Install Game" to update, which is not intuitive
  - Tab switching within the page requires additional state management

### Option B: Dedicated Update Game Page

- **Pros**:
  - Clean separation of concerns; update has its own page, hook, and component
  - Intuitive navigation: users see "Update Game" in the sidebar and know exactly where to go
  - Simpler state management per page (no shared state between install and update)
  - Easier to evolve independently (batch updates, update history) without bloating the Install page
  - Better for discoverability, especially for Steam Deck users navigating with a controller
- **Cons**:
  - Requires a new `AppRoute` variant (`'update'`), new sidebar item, new page component
  - More files to create (page, hook, types, Tauri commands) though the individual files are simpler
  - Slightly increases the sidebar item count (from 6 to 7, which is still manageable)
  - Some component duplication with Install (Proton selector, log display) unless shared components are extracted

### Option C: Profile Action (from Profile Editor Context Menu)

- **Pros**:
  - The most contextual approach: user is already looking at a profile and can directly trigger an update for it
  - No additional navigation; the action is available wherever profiles are displayed
  - Profile context (prefix, Proton path) is already loaded
- **Cons**:
  - The profile editor (`ProfileFormSections.tsx`) is already the largest component (~700+ lines); adding update workflow increases complexity
  - Requires a modal or inline expansion to house the update form, which is awkward for a multi-step operation
  - Harder to add features like batch updates or update history in a modal context
  - Less discoverable for new users who do not know to look for it in the profile actions
  - The current `ProfileActions.tsx` is minimal (~40 lines with delete/rename); adding update changes its scope

### Recommendation

**Option B (Dedicated Update Game Page)** is the strongest choice for the following reasons:

1. The manual workflow being replaced is a standalone operation ("I have an update .exe and want to run it in the right prefix"). It deserves its own page rather than being buried inside Install.
2. The codebase already has a clean page-per-route architecture (`ProfilesPage`, `LaunchPage`, `InstallPage`, etc.). A new `UpdatePage` follows the established pattern exactly.
3. The update flow is simpler than install (no prefix provisioning, no candidate discovery, no profile generation), so the dedicated page will be smaller and more focused than the Install page.
4. Future enhancements (batch updates, update history, community update patches) are easier to add to a dedicated page than to an Install page sub-section.
5. Sidebar item count goes from 6 to 7, which remains manageable. The "Update Game" item naturally belongs in the "Setup" section alongside "Install Game".

If the team prefers to start smaller, **Option A** (Install page sub-section) works as a Phase 1 stepping stone that can be promoted to Option B later. The backend code is identical either way.

## Task Breakdown Preview

### Phase 1: Foundation (Backend)

- **Task group**: Core Rust module for update-game
  - Create `crosshook-core/src/update/mod.rs`, `models.rs`, `service.rs`
  - `UpdateGameRequest`: profile_name, update_exe_path, proton_path (optional override), prefix_path (optional override)
  - `UpdateGameResult`: succeeded, message, helper_log_path, exit_code
  - `validate_update_request`: validate profile exists (via `ProfileStore`), prefix directory exists + is writable, update exe exists + is `.exe`, Proton path is executable
  - `update_game`: load profile, resolve prefix, build Proton command, run update exe, capture exit status
  - Unit tests for validation and command building
- **Parallel opportunity**: This can be built entirely independently of the frontend work

- **Task group**: Tauri command layer
  - Create `src-tauri/src/commands/update.rs` with `update_game` and `validate_update_request` commands
  - Register commands in `src-tauri/src/lib.rs` `invoke_handler`
  - Add log streaming (reuse or adapt `spawn_log_stream` from `commands/launch.rs`)
  - Consider a dedicated `update-log` Tauri event to avoid mixing with `launch-log`

### Phase 2: Core Implementation (Frontend)

- **Task group**: TypeScript types and hook
  - Create `src/types/update.ts` with `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameStage`, `UpdateGameValidationError`
  - Create `src/hooks/useUpdateGame.ts` modeled after `useInstallGame.ts` but simpler (no candidate discovery, no prefix resolution, no review profile)
  - The hook manages: request state, validation state, stage progression (idle -> preparing -> running -> completed/failed), result, error

- **Task group**: UI page and routing
  - Create `src/components/pages/UpdatePage.tsx` with profile selector, update exe browser, optional Proton override, run button, status display
  - Add `'update'` to the `AppRoute` union in `Sidebar.tsx`
  - Add an `UpdateIcon` to `SidebarIcons.tsx`
  - Add the update route to `SIDEBAR_SECTIONS`, `ROUTE_LABELS`, `VALID_APP_ROUTES`
  - Add `UpdatePage` case to `ContentArea.tsx` switch
  - The profile selector should use `invoke('profile_list')` and populate prefix/Proton from the selected profile
  - Reuse `ProtonPathField` from `InstallGamePanel.tsx` (extract to shared component if not already)

- **Parallel opportunity**: Types/hook and UI page can be developed in parallel once the backend commands are available

### Phase 3: Integration and Testing

- **Task group**: End-to-end validation
  - Manual test: select a profile, choose an update exe, run it, verify prefix is updated correctly
  - Verify log streaming appears in console drawer
  - Verify error handling for missing prefix, missing exe, non-executable Proton
  - Verify gamepad navigation on the new page (existing `useGamepadNav` should work automatically)
  - Add `cargo test -p crosshook-core` tests for the new update module

- **Task group**: Polish
  - Disable Launch button for a profile while its update is running (coordinate via Tauri managed state or events)
  - Clear/informative error messages for common failure modes
  - Ensure the page reset clears all state cleanly
  - CSS styling consistent with Install page layout patterns

### Critical Path Dependencies

1. Backend models and service must be complete before Tauri commands can be written.
2. Tauri commands must be registered before the frontend hook can invoke them.
3. The `AppRoute` type change in `Sidebar.tsx` triggers required updates in `App.tsx` and `ContentArea.tsx` simultaneously (TypeScript will enforce this via the exhaustive switch).
4. The profile selector on the update page depends on the existing `profile_list` command (already available).

## Key Decisions Needed

- **Routing**: Dedicated page (Option B) vs. Install page sub-section (Option A)? Recommendation is Option B, but the team may prefer starting with Option A as a stepping stone.
- **Event naming**: Reuse `launch-log` event for update log streaming (automatic console drawer integration, possible log mixing) vs. new `update-log` event (cleaner separation, requires console drawer changes)?
- **Profile loading**: Should the `update_game` backend command accept a profile name and load it internally, or should the frontend load the profile and pass prefix/Proton paths explicitly? Internal loading is safer (backend validates the profile exists), while explicit passing gives the frontend more control (user overrides).
- **Prefix backup**: Is pre-update backup a Phase 1 requirement or can it wait for Phase 2? Given the risk of prefix corruption, a lightweight backup option in Phase 1 would be prudent.

## Open Questions

- Does the user want the update flow to support `steam_applaunch`-style prefixes (where `compatdata_path` contains a `pfx` subdirectory) in addition to standalone `proton_run` prefixes? The existing `resolve_wine_prefix_path` helper handles both, so technical support is free, but the UI needs to know which profile fields to display.
- Should the update flow support running multiple executables in sequence (e.g., a patch that requires running `patch1.exe` then `patch2.exe`)? This would change the request model from a single `update_exe_path` to a `Vec<String>`.
- What is the expected behavior if the update executable modifies the game executable path (e.g., moves the game to a different directory within the prefix)? Should the profile be updated automatically, or should the user be prompted to re-confirm?
- Is there a known directory convention for where users store their update executables, or is it always ad-hoc browsing? This affects whether auto-detection is worth building.

## Relevant Files

### Backend (Rust)

- `crates/crosshook-core/src/install/service.rs` -- The `build_install_command` function (line 102-119) is the primary template for `build_update_command`; reuses `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`
- `crates/crosshook-core/src/install/models.rs` -- Template for `UpdateGameRequest` / `UpdateGameResult` models; shows the validation error enum pattern
- `crates/crosshook-core/src/launch/runtime_helpers.rs` -- Provides all the command-building primitives: `new_direct_proton_command` (line 12), `apply_runtime_proton_environment` (line 62), `resolve_wine_prefix_path` (line 94), `attach_log_stdio` (line 139)
- `crates/crosshook-core/src/launch/env.rs` -- Environment variable constants (`REQUIRED_PROTON_VARS`, `WINE_ENV_VARS_TO_CLEAR`) needed for correct prefix targeting
- `crates/crosshook-core/src/profile/models.rs` -- `GameProfile` struct with `RuntimeSection` (line 142) and `SteamSection` (line 119) that provide prefix and Proton paths
- `crates/crosshook-core/src/profile/toml_store.rs` -- `ProfileStore` for loading profiles by name
- `crates/crosshook-core/src/steam/proton.rs` -- Proton discovery and resolution for the optional Proton override selector
- `src-tauri/src/commands/install.rs` -- Template for `update.rs` Tauri commands; shows the `create_log_path` and `spawn_blocking` patterns
- `src-tauri/src/commands/launch.rs` -- `spawn_log_stream` function (line 103) for real-time log streaming via Tauri events
- `src-tauri/src/lib.rs` -- Command registration in `invoke_handler` (line 69-104)

### Frontend (React/TypeScript)

- `src/components/InstallGamePanel.tsx` -- Template for the update panel UI; shows form layout, `InstallField`, `ProtonPathField` components
- `src/components/pages/InstallPage.tsx` -- Template for `UpdatePage.tsx`; shows the page-level state management and Proton install loading pattern
- `src/hooks/useInstallGame.ts` -- Template for `useUpdateGame.ts`; the update hook will be significantly simpler (no candidate discovery, no prefix resolution, no review profile)
- `src/types/install.ts` -- Template for `src/types/update.ts` type definitions
- `src/types/profile.ts` -- `GameProfile` interface for reading prefix/Proton from selected profile
- `src/types/launch.ts` -- `LaunchRequest` interface shows how Proton env fields are structured
- `src/components/layout/Sidebar.tsx` -- `AppRoute` type union (line 12) and `SIDEBAR_SECTIONS` (line 32) need updates for a new route
- `src/components/layout/ContentArea.tsx` -- Exhaustive route switch (line 34-51) needs an `UpdatePage` case
- `src/App.tsx` -- `VALID_APP_ROUTES` record (line 14-21) needs the new route key
