# Feature Spec: Update Game

## Executive Summary

This feature adds game update/patch capability to CrossHook by allowing users to run a Windows update executable against an existing Proton prefix through the same infrastructure used by Install Game. Users select an existing profile (which provides the prefix path and Proton version), browse to an update `.exe`, and apply it — replacing the current manual script-based workflow of constructing environment variables and invoking `proton run` by hand. The implementation reuses 90%+ of existing infrastructure (`new_direct_proton_command`, `apply_runtime_proton_environment`, `resolve_wine_prefix_path` from `launch/runtime_helpers.rs`) and follows the established `install` module pattern but is significantly simpler: no prefix provisioning, no executable discovery, no profile generation. The primary risks are prefix corruption from failed updates and wrong-prefix targeting, both mitigated by requiring profile selection (which pins the prefix) and pre-flight validation.

## External Dependencies

### APIs and Services

#### Proton CLI (`proton run`)

- **Documentation**: [ValveSoftware/Proton GitHub](https://github.com/ValveSoftware/Proton) | [DeepWiki: Wine Prefix Management](https://deepwiki.com/ValveSoftware/Proton/2.2-wine-prefix-management)
- **Authentication**: None — local filesystem binary
- **Key Commands**:
  - `proton run <exe>`: Launches an executable within the configured prefix. CrossHook's default verb for `proton_run` launches.
  - `proton waitforexitandrun <exe>`: Same as `run` but explicitly waits for wineserver shutdown. Preferred for blocking update operations.
- **Constraints**:
  - Prefix must not be in use — Proton acquires `pfx.lock` and will block indefinitely if the game is running
  - Proton version mismatch may trigger automatic prefix upgrades (usually benign)
  - Some updaters are sensitive to working directory

#### Required Environment Variables

| Variable                           | Purpose                            | How CrossHook Sets It                                  |
| ---------------------------------- | ---------------------------------- | ------------------------------------------------------ |
| `STEAM_COMPAT_DATA_PATH`           | Compatdata root (parent of `pfx/`) | `runtime_helpers::apply_runtime_proton_environment()`  |
| `WINEPREFIX`                       | Actual Wine prefix directory       | Same function, via `resolve_wine_prefix_path()`        |
| `STEAM_COMPAT_CLIENT_INSTALL_PATH` | Steam client installation          | `runtime_helpers::resolve_steam_client_install_path()` |

### Libraries and SDKs

| Library                     | Version | Purpose                                          | Installation              |
| --------------------------- | ------- | ------------------------------------------------ | ------------------------- |
| `tokio`                     | 1.x     | Async process spawning, `block_on(child.wait())` | Already in crosshook-core |
| `serde`                     | 1.x     | Serialization for IPC types                      | Already in crosshook-core |
| `tracing`                   | 0.1     | Structured logging                               | Already in crosshook-core |
| `@tauri-apps/api/core`      | 2.x     | Frontend `invoke()` calls                        | Already in frontend       |
| `@tauri-apps/plugin-dialog` | 2.x     | File browser dialog                              | Already in frontend       |

**No new dependencies are required.** The existing crate and npm dependency set covers everything.

### External Documentation

- [Run .exe in existing Proton prefix (community gist)](https://gist.github.com/michaelbutler/f364276f4030c5f449252f2c4d960bd2): Shell command reference for manual prefix targeting
- [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ): Environment variable reference
- [protontricks](https://github.com/Matoking/protontricks): Complementary tool (not a dependency) for prefix manipulation

## Business Requirements

### User Stories

**Primary User: Linux Gamer / Steam Deck User**

- As a Linux gamer with a Proton-based game managed by CrossHook, I want to apply a game update through CrossHook so that I do not need to manually construct a Proton command line pointing at the correct prefix.
- As a Steam Deck user, I want to apply game updates from a controller-friendly UI so that the update workflow matches the install workflow I already know.
- As a user who maintains multiple game profiles, I want to select an existing profile and have CrossHook automatically resolve the prefix path and Proton version so that I never target the wrong prefix.
- As a user applying sequential patches (e.g., v1.0 → v1.1 → v1.2), I want a predictable workflow that preserves my profile settings between updates.

**Secondary User: Power User / Modder**

- As a power user, I want the option to override the Proton version or prefix path at update time so I can test compatibility without permanently changing my profile.
- As a modder, I want to see the update log output in the console drawer to diagnose update failures.

### Business Rules

1. **Profile Selection Required**: An update must target an existing saved profile. The profile provides the prefix path and Proton path.
   - Validation: Selected profile must exist in `ProfileStore.list()`
   - Exception: None — unlike Install Game which creates a new profile, Update Game always operates on an existing one

2. **Prefix Must Exist**: The profile's prefix path must already exist on disk. Updates do not create prefixes.
   - Validation: `Path::is_dir()` on the resolved prefix path
   - Exception: If the prefix was deleted externally, the user must reinstall first

3. **Update Executable Must Be a Valid Windows .exe**: Same `is_windows_executable()` check as install flow.
   - Validation: File exists, is a file, has `.exe` extension

4. **Profile Settings Are Read-Only During Update**: The update process must NOT modify the saved profile. The profile TOML on disk stays unchanged.
   - Validation: No `profile_save` call during the update flow

5. **Proton Path Resolution**: Defaults to the profile's configured Proton path but may be overridden by the user.
   - Validation: Same `is_executable_file()` check as install flow

6. **Single Update at a Time**: Only one update operation can run at a time (matches install's blocking pattern).

7. **`proton_run` Profiles Only**: Profiles with `launch.method == "native"` or `launch.method == "steam_applaunch"` are excluded from the profile selector. Steam games receive updates through Steam itself; only standalone Proton prefix profiles need manual update capability.

### Edge Cases

| Scenario                                   | Expected Behavior                                                    | Notes                                      |
| ------------------------------------------ | -------------------------------------------------------------------- | ------------------------------------------ |
| Prefix deleted externally                  | Validation error: "Prefix path does not exist. Reinstall the game."  | Caught before execution                    |
| Game is running during update              | Update may fail or corrupt files; warn user                          | Cannot reliably prevent on Linux           |
| Update requires specific DLLs/runtimes     | Update fails; direct user to log                                     | Out of scope for initial feature           |
| Update has GUI installer (InnoSetup, NSIS) | Proton renders the GUI; update blocks until process exits            | Standard Proton behavior                   |
| Update is a silent patcher                 | Working directory set to updater's parent directory                  | Matches `apply_working_directory` behavior |
| Steam-managed prefix (`compatdata`)        | `resolve_wine_prefix_path` handles `pfx/` subdirectory automatically | Already implemented in runtime_helpers     |
| Profile with empty prefix path             | Validation error: "Prefix path required"                             | Caught before execution                    |
| Multiple sequential patches                | Each is a separate "Apply Update" action; no batch mechanism         | Form retains profile after success         |

### Success Criteria

- [ ] User can select an existing `proton_run` profile from a dropdown
- [ ] Selecting a profile auto-fills prefix path and Proton path from the profile
- [ ] User can browse to a Windows `.exe` update/patch file
- [ ] Validation catches missing prefix, invalid executable, and invalid Proton path with clear messages
- [ ] Update executable runs through Proton against the correct prefix
- [ ] Update process exit code is captured and reported
- [ ] Log file is created and path is displayed to the user
- [ ] Profile on disk is NOT modified by the update process
- [ ] Native and `steam_applaunch` profiles are excluded from the profile selector
- [ ] UI matches the visual style and interaction patterns of the Install Game panel
- [ ] Gamepad/controller navigation works for the update form
- [ ] Rust tests exist for the update service validation and command building

## Technical Specifications

### Architecture Overview

```text
Frontend (React)                                    Backend (Rust)
+-------------------+   invoke('update_game')    +---------------------------+
| UpdateGamePanel   |  ----------------------->  | commands::update           |
|  (component)      |                            |   update_game()            |
|                   |   invoke('validate_       |   validate_update_request()|
|  useUpdateGame    |   update_request')         |                           |
|  (hook)           |  ----------------------->  +---------------------------+
+-------------------+                                      |
       |                                                   v
       |                                      +---------------------------+
       |  profile_load() (existing)           | crosshook_core::update    |
       | -----------------------------------> |   models.rs               |
       |                                      |   service.rs              |
       |  list_proton_installs() (existing)   |   (reuses launch/         |
       | -----------------------------------> |    runtime_helpers)        |
       +--                                    +---------------------------+
                                                           |
                                              +---------------------------+
                                              | Proton prefix (existing)  |
                                              |  drive_c/...              |
                                              +---------------------------+
```

### Data Models

#### UpdateGameRequest (Rust)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameRequest {
    pub profile_name: String,
    pub updater_path: String,
    pub proton_path: String,
    pub prefix_path: String,
    pub steam_client_install_path: String,
}
```

#### UpdateGameResult (Rust)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}
```

#### UpdateGameError / UpdateGameValidationError (Rust)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateGameError {
    Validation(UpdateGameValidationError),
    RuntimeUnavailable,
    LogAttachmentFailed { path: PathBuf, message: String },
    UpdaterSpawnFailed { message: String },
    UpdaterWaitFailed { message: String },
    UpdaterExitedWithFailure { status: Option<i32> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateGameValidationError {
    UpdaterPathRequired,
    UpdaterPathMissing,
    UpdaterPathNotFile,
    UpdaterPathNotWindowsExecutable,
    ProtonPathRequired,
    ProtonPathMissing,
    ProtonPathNotExecutable,
    PrefixPathRequired,
    PrefixPathMissing,
    PrefixPathNotDirectory,
}
```

#### TypeScript Types

```typescript
export interface UpdateGameRequest {
  profile_name: string;
  updater_path: string;
  proton_path: string;
  prefix_path: string;
  steam_client_install_path: string;
}

export interface UpdateGameResult {
  succeeded: boolean;
  message: string;
  helper_log_path: string;
}

export type UpdateGameStage = 'idle' | 'preparing' | 'running_updater' | 'complete' | 'failed';
```

### API Design (Tauri IPC)

#### `validate_update_request`

**Purpose**: Synchronous field-level validation before running the updater
**Authentication**: None (local IPC)

**Request**: `UpdateGameRequest`

**Response (200)**: `Result<(), String>`

**Errors**:

| Condition             | Error                             |
| --------------------- | --------------------------------- |
| Updater path empty    | `UpdaterPathRequired`             |
| Updater file missing  | `UpdaterPathMissing`              |
| Updater not a file    | `UpdaterPathNotFile`              |
| Updater not `.exe`    | `UpdaterPathNotWindowsExecutable` |
| Proton path empty     | `ProtonPathRequired`              |
| Proton path missing   | `ProtonPathMissing`               |
| Proton not executable | `ProtonPathNotExecutable`         |
| Prefix path empty     | `PrefixPathRequired`              |
| Prefix path missing   | `PrefixPathMissing`               |
| Prefix not directory  | `PrefixPathNotDirectory`          |

#### `update_game`

**Purpose**: Run a Windows update executable through Proton against an existing prefix. Blocks until the updater process exits.
**Authentication**: None (local IPC)

**Request**: `UpdateGameRequest`

**Response (200)**: `Result<UpdateGameResult, String>`

**Errors**: All validation errors above, plus `RuntimeUnavailable`, `LogAttachmentFailed`, `UpdaterSpawnFailed`, `UpdaterWaitFailed`, `UpdaterExitedWithFailure`

### System Integration

#### Files to Create

- `crates/crosshook-core/src/update/mod.rs`: Module root, re-exports
- `crates/crosshook-core/src/update/models.rs`: Request, Result, Error, ValidationError types
- `crates/crosshook-core/src/update/service.rs`: `update_game`, `validate_update_request`, `build_update_command`
- `src-tauri/src/commands/update.rs`: Tauri IPC commands
- `src/types/update.ts`: TypeScript types and validation maps
- `src/hooks/useUpdateGame.ts`: React hook for update flow state
- `src/components/UpdateGamePanel.tsx`: React UI component

#### Files to Modify

- `crates/crosshook-core/src/lib.rs`: Add `pub mod update;`
- `src-tauri/src/commands/mod.rs`: Add `pub mod update;`
- `src-tauri/src/lib.rs`: Register `update_game` and `validate_update_request` in `invoke_handler`
- `src/types/index.ts`: Add `export * from './update';`
- `src/components/pages/InstallPage.tsx`: Import and render `UpdateGamePanel` below `InstallGamePanel`

#### Files Reused As-Is

- `crates/crosshook-core/src/launch/runtime_helpers.rs`: All command-building primitives
- `src-tauri/src/commands/profile.rs`: `profile_load` and `profile_list`
- `src-tauri/src/commands/steam.rs`: `list_proton_installs`
- `src/utils/dialog.ts`: `chooseFile`, `chooseDirectory`

#### Configuration

- Log path convention: `update-<profile-slug>-<timestamp>.log` under `/tmp/crosshook-logs/`

## UX Considerations

### User Workflows

#### Primary Workflow: Apply Game Update

1. **Navigate to Install Game page**
   - User: Selects "Install Game" in sidebar
   - System: Page loads with Install Game shell and Update Game section below

2. **Select Profile**
   - User: Picks from dropdown of existing Proton-based profiles
   - System: Auto-populates prefix path, Proton path, and display name from profile. Shows read-only summary card.

3. **Select Update Executable**
   - User: Taps "Browse" to open file picker (`.exe` filter) or pastes a path
   - System: Validates file exists and has `.exe` extension. Displays file name.

4. **Pre-flight Validation**
   - System: Checks profile exists, prefix directory exists, executable valid, Proton path valid
   - System: Green "ready" indicator when all checks pass; inline errors for failures

5. **Confirm and Apply**
   - User: Presses "Apply Update"
   - System: Confirmation dialog: "Apply update to [Profile]? This will run [update.exe] inside the Proton prefix. This action cannot be automatically undone."
   - Default focus on "Cancel" to prevent accidental confirmation

6. **Execution**
   - System: Stage indicator "Running update..." with indeterminate progress bar
   - System: Console output streams in ConsoleDrawer via `launch-log` events

7. **Completion**
   - Success: Green status card with "Update applied successfully" + log path
   - Failure: Red error message + log path + "Retry" button

#### Error Recovery Workflow

1. **Error Occurs**: Update process exits with non-zero code
2. **User Sees**: Red error message with exit code and log path in status card
3. **Recovery**: User reviews console log, fixes issue (e.g., installs prerequisite into prefix), clicks "Retry"

### UI Patterns

| Component        | Pattern                           | Notes                                                |
| ---------------- | --------------------------------- | ---------------------------------------------------- |
| Profile selector | `ThemedSelect` dropdown           | Filtered to `proton_run` profiles only               |
| Update exe field | `InstallField` with Browse button | Reuse from InstallGamePanel (extract to shared)      |
| Proton path      | `ProtonPathField`                 | Reuse from InstallGamePanel (extract to shared)      |
| Status card      | `crosshook-install-card` pattern  | Stage indicator, status text, hint text, log path    |
| Confirmation     | Modal dialog                      | Task-specific button labels, default focus on Cancel |
| Progress         | Indeterminate bar + elapsed timer | CSS animation on `crosshook-progress` element        |

### Accessibility Requirements

- All interactive elements reachable via D-pad focus navigation (`useGamepadNav` hook)
- Minimum 48px touch targets (matching `--crosshook-touch-target-min`)
- Browse button as primary file selection (avoids virtual keyboard on Steam Deck)
- Focus indicators visible in dark theme (existing `focus.css` styles)
- Confirmation dialog navigable via gamepad (A-button confirms, B-button cancels)

### Performance UX

- **Loading States**: "Loading profiles..." placeholder in select while backend responds
- **Optimistic Updates**: None — update operations are blocking and must complete before reporting
- **Error Feedback**: Inline field-level errors appear immediately on validation; general errors in status card
- **Console Output**: Auto-expand ConsoleDrawer on update start; clear previous lines; scroll-to-bottom

## Recommendations

### Implementation Approach

**Recommended Strategy**: Build update-game as a new `update` module in `crosshook-core`, co-located on the existing Install page as a section below the Install Game panel.

**Phasing:**

1. **Phase 1 - MVP**: Core update execution
   - Rust: `update` module with models, service, validation
   - Tauri: `update_game` and `validate_update_request` commands with real-time log streaming via `update-log` Tauri event (using `spawn_log_stream` pattern from launch)
   - Frontend: `useUpdateGame` hook + `UpdateGamePanel` component on Install page
   - ConsoleDrawer subscribes to `update-log` event for live output
   - Elapsed time display during execution

2. **Phase 2 - Enhancement**: Safety and polish
   - Pre-update prefix backup (tar snapshot of `drive_c`)
   - Update history log (append-only TOML)

3. **Phase 3 - Polish**: Advanced features
   - Batch updates (queue multiple profile/update-exe pairs)
   - Community tap integration (update patches in tap manifests)
   - Post-update verification (re-run candidate discovery)
   - Working directory override field

### Technology Decisions

| Decision          | Choice                                             | Rationale                                                             |
| ----------------- | -------------------------------------------------- | --------------------------------------------------------------------- |
| Backend module    | New `update` module, sibling to `install`          | Keeps concerns separated; update is simpler than install              |
| Command builder   | Reuse `runtime_helpers` primitives                 | Already tested, handles prefix resolution                             |
| Execution model   | Real-time streaming via `spawn_log_stream`         | Better UX — live console output during update; matches launch pattern |
| UI placement      | Section on Install page                            | Related operations together; no new routes needed                     |
| Profile scope     | `proton_run` profiles only                         | Steam games update through Steam; only standalone prefixes need this  |
| Profile loading   | Frontend loads profile, passes paths               | User can override fields before applying                              |
| Shared components | Extract `InstallField`/`ProtonPathField` to shared | Removes duplication, ensures consistent styling                       |

### Quick Wins

- **Profile-aware prefix resolution**: Profiles already store prefix path — skip all prefix discovery logic
- **Reuse `resolve_wine_prefix_path`**: Both `compatdata` and standalone prefixes work without special casing
- **Log path convention**: `update-<slug>-<timestamp>.log` under `/tmp/crosshook-logs/` — picked up by existing infrastructure

### Future Enhancements

- **Prefix backup before update**: `tar czf` snapshot of `drive_c` for rollback capability
- **Auto-detect update executables**: Scan configurable directory for `.exe` files matching known patterns
- **Dedicated Update page**: Promote from Install page sub-section to first-class sidebar route if usage justifies it
- **MSI support**: Detect `.msi` extensions and auto-prepend `msiexec.exe /i`

## Risk Assessment

### Technical Risks

| Risk                                            | Likelihood | Impact   | Mitigation                                                                                        |
| ----------------------------------------------- | ---------- | -------- | ------------------------------------------------------------------------------------------------- |
| Prefix corruption from failed update            | Medium     | High     | Phase 2 prefix backup; always log full session                                                    |
| Wrong prefix targeted                           | Low        | Critical | Require profile selection (never free-text prefix); validate prefix exists and contains `drive_c` |
| Proton version mismatch triggers prefix upgrade | Medium     | Medium   | Default to profile's Proton version; warn if user selects different version                       |
| File permission issues                          | Low        | Medium   | Validate prefix is writable before starting                                                       |
| Update executable needs interactive GUI         | High       | Low      | Expected; Proton renders GUI windows natively                                                     |
| Update process hangs indefinitely               | Low        | Medium   | Document: user must kill process externally. Future: add timeout/cancel button                    |
| Race condition: game launched during update     | Low        | Medium   | Disable Launch button for profile while update running                                            |

### Integration Challenges

- **Tauri command registration**: New commands must be added to `invoke_handler` in `src-tauri/src/lib.rs`
- **Event channel**: Dedicated `update-log` event requires ConsoleDrawer to subscribe to both `launch-log` and `update-log`
- **Shared component extraction**: `InstallField` and `ProtonPathField` currently live in `InstallGamePanel.tsx`; extracting them is a prerequisite for clean reuse

### Security Considerations

- All executables run inside a Proton sandbox (not native) — same attack surface as Install Game
- Display full executable path for user review before confirmation
- Never auto-execute without user confirmation

## Task Breakdown Preview

### Phase 1: Foundation (Backend)

**Focus**: Core Rust module and Tauri commands
**Tasks**:

- Create `update` module: `mod.rs`, `models.rs`, `service.rs`
- Implement `validate_update_request` and `update_game` service functions
- Implement `build_update_command` reusing runtime_helpers
- Create `src-tauri/src/commands/update.rs` with Tauri commands
- Register commands in `lib.rs`
- Write unit tests for validation and command building
  **Parallelization**: Backend module and Tauri commands can be developed together

### Phase 2: Core Implementation (Frontend)

**Focus**: TypeScript types, React hook, and UI component
**Dependencies**: Phase 1 must complete (Tauri commands must exist)
**Tasks**:

- Create `src/types/update.ts` with types and validation maps
- Create `src/hooks/useUpdateGame.ts` (state machine: idle → preparing → running → complete/failed)
- Extract `InstallField`/`ProtonPathField` to shared components
- Create `src/components/UpdateGamePanel.tsx`
- Integrate into `InstallPage.tsx` below `InstallGamePanel`
  **Parallelization**: Types/hook and UI component can be developed in parallel once backend is available

### Phase 3: Integration and Testing

**Focus**: End-to-end validation and polish
**Dependencies**: Phase 2 must complete
**Tasks**:

- Manual test: select profile, choose update exe, run, verify prefix updated
- Verify error handling for all validation errors
- Verify gamepad navigation
- CSS styling consistency with Install page
- Add `cargo test -p crosshook-core` tests for update module

## Decisions (Resolved)

1. **UI Placement** → **Section on Install page**
   - Co-locate Update Game as a section below the Install Game panel on the existing Install page. No new sidebar route.

2. **Log Streaming** → **Real-time streaming from Phase 1**
   - Use the same `spawn_log_stream` pattern from `commands/launch.rs` so users see live output in the ConsoleDrawer during the update. This replaces the blocking `spawn_blocking + block_on` pattern.

3. **Event Channel** → **New `update-log` event**
   - Dedicated event to avoid mixing with `launch-log` when both are active. ConsoleDrawer must subscribe to both events.

4. **Working Directory Default** → **Updater's parent directory**
   - The game path inside the prefix should not change during an update. The updater's parent directory is the correct default. No override field needed.

5. **`steam_applaunch` Profile Support** → **`proton_run` only**
   - Steam games receive updates through Steam itself. Only standalone `proton_run` profiles need manual update capability. Filter the profile selector to exclude both `native` and `steam_applaunch` profiles.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Proton CLI, environment variables, prefix management, library assessment
- [research-business.md](./research-business.md): User stories, business rules, workflows, codebase integration analysis
- [research-technical.md](./research-technical.md): Architecture design, data models, API contracts, system constraints
- [research-ux.md](./research-ux.md): User workflows, competitive analysis, gamepad navigation, error handling UX
- [research-recommendations.md](./research-recommendations.md): Implementation approach, risk assessment, alternative approaches, task breakdown
