# Business Logic Research: UI Enhancements

## Executive Summary

CrossHook's current UI crams profile editing, launch controls, launcher export, and a console log viewer into a single two-column Main tab, forcing users to context-switch between unrelated tasks and visually overloading the 1280x800 Steam Deck viewport. The three horizontal tabs (Main, Settings, Community) mix operational depth (profile editing is 500+ lines of component code) with shallow utility panels (Settings is read-mostly), while the Profile/Install Game sub-tabs inside ProfileEditor create a nested tab-within-tab pattern that confuses the information hierarchy. A reorganization around workflow-centric vertical navigation -- grouping features by what the user is trying to accomplish rather than by technical category -- would reduce cognitive load, improve gamepad navigability, and make each view feel purposeful.

## User Stories

### Primary User: Steam Deck Gamer

- As a Steam Deck gamer, I want to load an existing profile and launch my game + trainer with minimal navigation so that I can start playing quickly from the couch.
- As a Steam Deck gamer, I want launch status and console output visible while I wait for my game to boot so that I know whether to proceed with the trainer step.
- As a Steam Deck gamer, I want gamepad-friendly navigation with large touch targets and clear focus indicators so that I do not need to reach for a keyboard or mouse.
- As a Steam Deck gamer, I want to install a new Windows game through a guided flow without needing to understand which sub-tab or panel I need next.
- As a Steam Deck gamer, I want to browse community profiles and import one into my library with a single action so that I skip manual configuration entirely.

### Primary User: Desktop Linux Power User

- As a power user, I want to edit profile details (paths, runner method, Steam metadata) and see the effect on the launch panel in real time so that I can verify my configuration before launching.
- As a power user, I want to export a launcher script and desktop entry from my current profile without leaving the profile context so that the export always reflects the latest edits.
- As a power user, I want to manage all exported launchers in one place (list, re-export stale, delete) so that I do not have orphaned files on disk.
- As a power user, I want to adjust app settings (auto-load, profiles directory) and review recent file history without losing my place in the profile editor.
- As a power user, I want the install game flow to feel like a distinct wizard, not a tab swap inside the profile editor, so that I understand the separate lifecycle (install -> review -> save -> normal profile).

## Business Rules

### Core Rules

1. **Profile Must Have Executable Path Before Save**: A profile cannot be saved unless `game.executable_path` is non-empty. This is enforced in `useProfile.ts` at `validateProfileForSave`.
   - Validation: Check `profile.game.executable_path.trim().length > 0` before enabling Save.
   - Exception: None. This is a hard requirement.

2. **Launch Method Determines Visible Fields**: The selected `launch.method` (steam_applaunch, proton_run, native) determines which runtime fields are shown in `ProfileFormSections`. Steam shows App ID, compatdata, proton path, and auto-populate. Proton shows prefix, proton path, working directory. Native shows only working directory.
   - Validation: `resolveLaunchMethod()` in `App.tsx` and `useProfile.ts` derives the effective method from profile state.
   - Exception: Install Game context always forces `proton_run`.

3. **Install Game Context Overrides Launch Method**: When `profileEditorTab === 'install'`, the `effectiveLaunchMethod` is forced to `proton_run` and the LaunchPanel request is set to `null` (disabling launch buttons). This is computed in `App.tsx` lines 85-91.
   - Validation: The `effectiveLaunchMethod` memo in App.tsx.
   - Exception: None.

4. **Launcher Export Requires Trainer + Runtime Paths**: Export is disabled unless trainer_path, prefix_path, and proton_path are all non-empty (plus steam_app_id for steam_applaunch). Checked in `LauncherExport.tsx` via `canExport`.
   - Validation: `canExport` computed boolean.
   - Exception: LauncherExport shows an install-context informational panel when `context === 'install'`.

5. **Launcher Export Only Shows for Non-Native Methods**: `shouldShowLauncherExport` in App.tsx is true when the effective method is steam_applaunch or proton_run, or when the install editor context is active. Native launch profiles do not show launcher export.
   - Validation: App.tsx lines 96-99.
   - Exception: The install tab always shows it (as informational).

6. **Profile Delete Cascades to Launcher Files**: When deleting a profile, the system first checks for associated launcher files via `check_launcher_for_profile`. If found, a confirmation dialog shows which files will also be removed.
   - Validation: `confirmDelete` in `useProfile.ts` calls the check before showing the pending delete dialog.
   - Exception: If the launcher check fails, the error is shown and delete is blocked.

7. **Install Review Draft is Independent of Profile Editor State**: The install review modal manages its own `ProfileReviewSession` state with `draftProfile`, `originalProfile`, and dirty checking. Saving the review draft calls `persistProfileDraft` which saves and then switches the user to the Profile tab.
   - Validation: `isProfileReviewSessionDirty()` compares draft to original.
   - Exception: Dirty drafts prompt confirmation dialogs before being replaced or hidden.

8. **Two-Step Launch Flow**: For steam_applaunch and proton_run, launching is a two-phase process: (1) Launch Game -> (2) Wait for trainer -> Launch Trainer. The `LaunchPhase` enum tracks: Idle -> GameLaunching -> WaitingForTrainer -> TrainerLaunching -> SessionActive.
   - Validation: `useLaunchState` reducer manages all transitions.
   - Exception: Native launch goes directly to SessionActive after game launch.

9. **Auto-Load Last Profile on Startup**: When `auto_load_last_profile` is enabled in settings, the Tauri backend emits an `auto-load-profile` event that the frontend listens for in `App.tsx` lines 201-208.
   - Validation: Setting is persisted in `settings.toml` via `settings_save`.
   - Exception: If the profile file no longer exists, load fails silently.

10. **Community Taps Persist in Settings**: Community tap subscriptions are stored in `AppSettingsData.community_taps` and synced through the backend. Add/remove operations immediately persist to settings.
    - Validation: `useCommunityProfiles` saves taps on every add/remove.
    - Exception: None.

### Edge Cases

- **Profile Name Typed But Not Loaded**: User types a name that matches an existing profile but does not select it from the dropdown. The profile data remains empty/stale. `profileExists` check uses `profiles.includes(profileName.trim())`.
- **Install Review Session Persists Across Tab Switches**: The `profileReviewSession` state lives in `ProfileEditorView` and survives switching between Profile and Install sub-tabs. A dirty review session prompts confirmation when the user tries to start a new install or open a different review result.
- **Stale Launcher Detection**: `LauncherExport` checks `launcherStatus.is_stale` to show a warning when exported launcher files are out of date with the current profile, offering a re-export button.
- **Gamepad Back Button Closes Modals**: `handleGamepadBack` in App.tsx finds the last modal close button via `[data-crosshook-focus-root="modal"] [data-crosshook-modal-close]` and clicks it. This depends on modal DOM ordering.
- **Proton Install Duplication**: Both `ProfileEditor` and `InstallGamePanel` independently load and sort proton installs. This is duplicated state that could be lifted.

## Workflows

### Primary Workflow: Load Profile and Launch Game

1. User opens app. If auto-load is enabled, the last profile loads automatically.
2. User sees the Main tab with ProfileEditorView on the left, LaunchPanel on the right.
3. User selects a profile from the Load Profile dropdown (or types a name).
4. System loads the profile from disk, populates all fields, syncs last-used-profile metadata.
5. User reviews the launch method and confirms paths are correct.
6. User clicks "Launch Game" on the LaunchPanel.
7. System dispatches `launch_game` IPC. Phase transitions to GameLaunching, then WaitingForTrainer.
8. User waits for game to reach main menu (console streams logs).
9. User clicks "Launch Trainer" on LaunchPanel.
10. System dispatches `launch_trainer` IPC. Phase transitions to SessionActive.
11. User plays the game with trainer active.

**Navigation cost**: Steps 2-6 require scanning a dense two-column layout. The LaunchPanel is on the right, profile fields on the left, console at the bottom. On Steam Deck (1280x800), this is cramped.

### Secondary Workflow: Create New Profile

1. User types a new profile name in the Profile Name field.
2. User fills Game Path (browse dialog), Trainer Path, selects Runner Method.
3. Conditional: If steam_applaunch, user fills App ID, compatdata path, proton path (or uses Auto-Populate).
4. Conditional: If proton_run, user fills prefix path, proton path.
5. User optionally fills Launcher Name and Icon Path.
6. User clicks Save.
7. System validates, normalizes, saves to TOML, refreshes profile list.

**Pain point**: All fields are visible at once regardless of runner method. The form does hide irrelevant fields per method, but the overall visual density is high.

### Tertiary Workflow: Install Windows Game

1. User clicks the "Install Game" sub-tab inside ProfileEditorView.
2. System shows InstallGamePanel with guided form fields.
3. User enters Profile Name, Installer EXE path, optional Trainer EXE, selects Proton version.
4. System auto-resolves default prefix path based on profile name (debounced 250ms).
5. User clicks "Install Game".
6. System validates, creates prefix if needed, launches installer through Proton.
7. Installer runs. Phase shows "Running installer". Console streams logs.
8. Installer completes. System discovers executable candidates.
9. ProfileReviewModal auto-opens showing the generated profile.
10. User reviews, selects final executable from candidates, edits fields as needed.
11. User clicks "Save Profile" in the modal footer.
12. System saves the profile, switches to Profile tab, loads the saved profile.

**Pain point**: The install flow is nested inside the profile editor via a sub-tab, making it feel like a minor feature instead of a distinct workflow. The modal popup for review is visually disconnected from the install form. The right-side panels (LaunchPanel, LauncherExport) show install-context informational placeholders that are not actionable.

### Workflow: Export Launcher

1. User has a loaded profile with trainer and runtime paths configured.
2. User views LauncherExport panel on the right side of the Main tab.
3. System derives launcher name from profile fields, checks launcher status on disk.
4. User optionally edits launcher name.
5. User clicks "Export Launcher".
6. System validates, generates shell script + desktop entry.
7. System refreshes launcher status, shows export result with paths.

**Pain point**: LauncherExport is visually stacked below LaunchPanel. On smaller screens, it may require scrolling. It shares the right column with an unrelated component (LaunchPanel).

### Workflow: Browse and Import Community Profile

1. User clicks the Community tab.
2. System loads taps from settings, refreshes profile index.
3. User optionally adds a tap URL (git repository) and syncs.
4. User searches/filters profiles by game name, trainer, compatibility rating.
5. User clicks "Import" on a profile card.
6. System imports the profile JSON into the local profiles directory.
7. User must manually switch to Main tab and load the imported profile.

**Pain point**: After importing, there is no automatic navigation to the profile editor. The Community and Main tabs are disconnected workflows.

### Error Recovery

- **Launch Failure**: Error message shown in LaunchPanel. Phase falls back to previous safe state (Idle or WaitingForTrainer). User clicks Reset or retries.
- **Save Failure**: Error banner shown below the Save button in ProfileEditorView. User can re-attempt save.
- **Install Failure**: Stage transitions to 'failed', error shown in install status card. User can adjust fields and click "Retry Install".
- **Review Draft Conflict**: When a new install result arrives while a dirty review draft is open, a confirmation dialog asks the user to replace or keep the current draft.
- **Export Validation Failure**: Error message shown in LauncherExport panel. User must fix profile fields.

## Domain Model

### Key Entities

- **GameProfile**: The central entity. Contains game info (name, executable_path), trainer info (path, type), injection config, steam config (app_id, compatdata, proton, launcher metadata), runtime config (prefix, proton, working_directory), and launch method. Defined in `types/profile.ts`.
- **LaunchRequest**: A derived, read-only snapshot built from a GameProfile for a specific launch invocation. Contains method, game_path, trainer_path, steam config, runtime config, and launch flags. Defined in `types/launch.ts`.
- **LaunchPhase**: State machine for the launch lifecycle: Idle -> GameLaunching -> WaitingForTrainer -> TrainerLaunching -> SessionActive. Enum in `types/launch.ts`.
- **LauncherInfo**: Represents an exported launcher on disk: display_name, slug, script_path, desktop_entry_path, existence booleans, is_stale flag. Defined in `types/launcher.ts`.
- **ProfileReviewSession**: Temporary session for reviewing an install-generated profile. Contains isOpen, source, profileName, original/draft profiles, candidate options, helper log path, install message, save error. Defined in `types/profile-review.ts`.
- **InstallGameRequest**: Input for the install flow: profile_name, display_name, installer_path, trainer_path, proton_path, prefix_path, installed_game_executable_path. Defined in `types/install.ts`.
- **InstallGameResult**: Backend response from install: succeeded, message, helper_log_path, profile_name, needs_executable_confirmation, discovered candidates, generated profile. Defined in `types/install.ts`.
- **CommunityProfileIndexEntry**: An entry from a community tap: tap_url, branch, manifest_path, relative_path, manifest (with metadata and profile). Defined in `hooks/useCommunityProfiles.ts`.
- **AppSettingsData**: Persisted settings: auto_load_last_profile, last_used_profile, community_taps. Defined in `types/settings.ts`.
- **RecentFilesData**: Persisted recent paths: game_paths, trainer_paths, dll_paths. Defined in `types/settings.ts`.

### State Transitions

- **Profile Lifecycle**: Empty -> Loading -> Loaded (clean) -> Editing (dirty) -> Saving -> Loaded (clean)
- **Launch Lifecycle**: Idle -> GameLaunching -> WaitingForTrainer -> TrainerLaunching -> SessionActive (for two-step). Idle -> GameLaunching -> SessionActive (for native).
- **Install Lifecycle**: idle -> preparing -> running_installer -> review_required -> ready_to_save. Any stage can transition to 'failed'.
- **Review Session Lifecycle**: null (no session) -> created (isOpen: true) -> editing (dirty) -> saved (null, returns to profile tab). Can be hidden (isOpen: false) without destroying.
- **Launcher Status**: Not Exported -> Exported -> Stale (profile changed) -> Re-exported. Exported -> Deleted.
- **Default Prefix Path Resolution**: idle -> loading -> ready | failed (driven by debounced profile name input).

### Relationships Between Entities That Should Be Visually Grouped

- **Profile + Launch + Console**: These are the core operational trio. Editing a profile should naturally lead to launching, and the console shows launch output. They are part of the same workflow.
- **Profile + Launcher Export**: Export derives from the current profile state. Changing the profile should be reflected in the export panel. These should remain visually connected.
- **Install Game + Review Modal**: The install flow is a standalone wizard that produces a profile. It should feel like its own section, not a sub-tab of the profile editor.
- **Community Browser + Compatibility Viewer**: These are both browsing/discovery features and belong together.
- **Settings + Manage Launchers + Recent Files**: Administrative/housekeeping features. Settings is mostly read-only configuration review.

## Existing Codebase Integration

### Component Coupling Analysis

**Tightly coupled -- must stay together or share state:**

- `App.tsx` and `ProfileEditorView`: App owns the `useProfile` hook state and passes it down. The profile state drives LaunchPanel, LauncherExport, and the heading text.
- `ProfileEditorView` and `ProfileFormSections`: ProfileFormSections is the actual form renderer. ProfileEditorView wraps it with tab switching, save/delete buttons, and the review modal.
- `ProfileEditorView` and `InstallGamePanel`: InstallGamePanel lives inside the ProfileEditorView's install sub-tab and communicates via `onOpenProfileReview` and `onRequestInstallAction` callbacks.
- `ProfileEditorView` and `ProfileReviewModal`: The review modal is rendered inside ProfileEditorView and shares review session state.
- `LaunchPanel` and `useLaunchState`: LaunchPanel is a thin view over the hook.
- `LauncherExport` and the current profile: LauncherExport reads profile, method, steamClientInstallPath, and targetHomePath from props.

**Loosely coupled -- can be separated:**

- `ConsoleView`: Completely independent. Listens to `launch-log` Tauri events. Has no props and no shared state. Can live anywhere.
- `SettingsPanel`: Receives data via props, no shared hooks. Self-contained.
- `CommunityBrowser` and `CompatibilityViewer`: CommunityBrowser manages its own state via `useCommunityProfiles`. CompatibilityViewer is a pure presentational component receiving entries as props.
- `AutoPopulate`: Receives callbacks and values via props. Could be extracted from ProfileFormSections context.

**State that would need to be lifted or shared in a new layout:**

- `useProfile` result: Currently created in `App.tsx` and passed to `ProfileEditorView`. If profile editing and launch panels are in different views, this hook's state must be accessible to both.
- `effectiveLaunchMethod` and `launchRequest`: Derived in `App.tsx` from profile state. LaunchPanel and LauncherExport both depend on these.
- `steamClientInstallPath` and `targetHomePath`: Derived in App.tsx, passed to both LauncherExport and SettingsPanel.

### Current UI Pain Points

- **Nested tab-in-tab pattern**: The Main tab contains Profile/Install Game sub-tabs inside ProfileEditorView. This creates a confusing hierarchy: top-level tabs -> content area -> sub-tabs -> content. Users must mentally track two tab levels.
- **Two-column layout wastes vertical space on Steam Deck**: The 1280x800 viewport with a two-column grid (`1.3fr / 0.9fr`) means the profile editor is squeezed horizontally while LaunchPanel and LauncherExport are stacked vertically on the right. Scrolling is often required.
- **Right panel serves dual purpose**: LaunchPanel and LauncherExport are stacked in the same column. When in install context, they show informational placeholders instead of actionable content, wasting space.
- **Heading text dynamically changes based on sub-tab and method**: `headingTitle` and `headingCopy` in App.tsx switch between "Two-step Steam launch", "Two-step Proton launch", "Native launch", and "Install Windows Game" based on context. This overloads the app header with context-dependent information.
- **Massive inline style usage**: LaunchPanel (255 lines), LauncherExport (656 lines), ConsoleView (271 lines), and AutoPopulate (320 lines) define most styles inline as CSSProperties objects rather than using CSS classes. This makes the components harder to maintain and creates visual inconsistency with the CSS-class-based components.
- **ProfileFormSections is reused in two contexts**: It renders both the normal profile editor form and the review modal form (with `reviewMode` prop). This dual-use creates complexity around which fields to show/collapse.
- **Disconnect after community import**: Importing a community profile does not navigate the user to the profile editor or auto-select the imported profile. The user must manually switch tabs and find the profile.

### Inline Style vs CSS Class Inconsistency

The codebase has two styling approaches that coexist:

- **CSS classes** (`theme.css`, `focus.css`): Used by ProfileEditorView, ProfileFormSections, InstallGamePanel, SettingsPanel. These follow the `crosshook-*` BEM-like naming convention.
- **Inline CSSProperties objects**: Used extensively by LaunchPanel, LauncherExport, ConsoleView, AutoPopulate, CommunityBrowser, CompatibilityViewer. These define styles as JavaScript objects (e.g., `panelStyles`, `inputStyle`, `buttonStyle`).

Components that use CSS classes are more maintainable and consistent. The inline-styled components duplicate colors, border radii, and spacing values that could reference CSS custom properties.

### Patterns to Follow

- **Hook-based state management**: Each domain area has its own hook (`useProfile`, `useLaunchState`, `useInstallGame`, `useCommunityProfiles`, `useGamepadNav`). This pattern should be preserved; new views can consume the same hooks.
- **Tauri invoke for all backend operations**: All IPC goes through `invoke()` from `@tauri-apps/api/core`. No direct filesystem or system calls in the frontend.
- **CSS custom properties in `variables.css`**: Design tokens for colors, spacing, radii, shadows, and fonts. New CSS should reference these variables.
- **BEM-like class naming**: `crosshook-component`, `crosshook-component--modifier`, `crosshook-component__element`. Used in theme.css.
- **Touch-target minimum**: `--crosshook-touch-target-min: 48px` ensures gamepad/touch friendliness.
- **Modal focus trapping**: `ProfileReviewModal` implements full accessibility: focus trapping, inert siblings, scroll lock, keyboard escape handling, and `data-crosshook-focus-root` for gamepad navigation.
- **Confirmation dialog pattern**: Both `ProfileEditorView` (delete confirmation) and `ProfileReviewModal` (review confirmation) use inline overlays with confirm/cancel actions.

### Components to Leverage

- **`useGamepadNav`**: Already provides full controller navigation (D-pad, analog stick, confirm/back buttons). Any new navigation structure must work with this hook's focus management. The hook scans for focusable elements within its root ref or any modal focus root.
- **`ConsoleView`**: Completely independent, can be placed in any view. Listens to `launch-log` events globally.
- **`ProfileFormSections`**: Already handles all three runner methods and review mode. Can be reused as-is in any new layout.
- **`ProfileReviewModal`**: Full-featured accessible modal with portal rendering, focus trapping, and confirmation sub-dialog. Can be used for other modal needs.
- **CSS class library**: `theme.css` already defines `.crosshook-panel`, `.crosshook-card`, `.crosshook-button` variants, `.crosshook-field`, `.crosshook-input`, modal classes, and responsive breakpoints. These should be the foundation for any new views.

## Relevant Files

- `src/crosshook-native/src/App.tsx`: Root shell, tab navigation, profile/settings/launch state orchestration, heading derivation
- `src/crosshook-native/src/components/ProfileEditor.tsx`: Profile sub-tab switching, save/delete UI, install review session management, review modal rendering
- `src/crosshook-native/src/components/ProfileFormSections.tsx`: Shared form renderer for profile editing and review mode, runner-method-conditional fields
- `src/crosshook-native/src/components/InstallGamePanel.tsx`: Guided install form, candidate selection, auto-review-open, proton install loading
- `src/crosshook-native/src/components/LaunchPanel.tsx`: Launch controls with two-step flow, install-context informational panel
- `src/crosshook-native/src/components/LauncherExport.tsx`: Export form, launcher status, delete, stale detection, install-context panel
- `src/crosshook-native/src/components/ConsoleView.tsx`: Independent log viewer, Tauri event listener
- `src/crosshook-native/src/components/SettingsPanel.tsx`: Settings form, manage launchers section, recent files display
- `src/crosshook-native/src/components/CommunityBrowser.tsx`: Tap management, profile search/filter, import
- `src/crosshook-native/src/components/CompatibilityViewer.tsx`: Compatibility database browser with filters
- `src/crosshook-native/src/components/AutoPopulate.tsx`: Steam auto-discovery for App ID, compatdata, proton path
- `src/crosshook-native/src/components/ProfileReviewModal.tsx`: Accessible modal with portal, focus trapping, confirmation sub-dialog
- `src/crosshook-native/src/hooks/useProfile.ts`: Profile CRUD state machine, save/load/delete, draft persistence
- `src/crosshook-native/src/hooks/useLaunchState.ts`: Launch phase state machine with reducer pattern
- `src/crosshook-native/src/hooks/useInstallGame.ts`: Install flow state, prefix resolution, validation, candidate management
- `src/crosshook-native/src/hooks/useGamepadNav.ts`: Gamepad polling, focus management, controller mode detection
- `src/crosshook-native/src/hooks/useCommunityProfiles.ts`: Community tap CRUD, sync, import
- `src/crosshook-native/src/styles/theme.css`: All CSS classes, modal styles, responsive breakpoints
- `src/crosshook-native/src/styles/variables.css`: Design tokens (colors, spacing, radii, fonts)
- `src/crosshook-native/src/styles/focus.css`: Gamepad/controller focus styles, touch targets
- `src/crosshook-native/src/types/`: All TypeScript type definitions (profile, launch, launcher, install, settings, profile-review)
- `src/crosshook-native/src-tauri/tauri.conf.json`: Window config (1280x800, dark theme, AppImage target)

## Success Criteria

- [ ] Every user workflow (load+launch, create profile, install game, export launcher, browse community) has a clear navigation path with no more than one level of tab nesting
- [ ] The primary workflow (load profile -> launch game -> launch trainer) requires at most 2 navigation actions from app open
- [ ] The install game flow is visually distinct from the profile editing flow, with its own dedicated view or wizard-like progression
- [ ] No inline CSSProperties objects remain in components; all styling uses CSS classes that reference `variables.css` tokens
- [ ] Console output is accessible from any view where launch or install operations are active
- [ ] Gamepad navigation (`useGamepadNav`) works correctly with the new layout structure, including focus cycling within views
- [ ] The 1280x800 viewport shows all critical information for the active workflow without horizontal scrolling; vertical scrolling is acceptable for detail-heavy views
- [ ] Community profile import optionally offers to navigate to the imported profile in the editor
- [ ] Launcher export state is visible from the profile context without requiring a separate navigation step
- [ ] The total number of top-level navigation sections is between 3 and 6, each representing a distinct user intent
