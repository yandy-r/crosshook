# UI Enhancements

CrossHook's React 18 + Tauri v2 frontend uses a horizontal tab layout (`App.tsx` manages `activeTab` state for Main/Settings/Community) where the Main tab renders a two-column grid with `ProfileEditorView` (which itself has Profile/Install sub-tabs), `LaunchPanel`, `LauncherExport`, and `ConsoleView` all competing for 1280x800 viewport space. The restructure replaces this with a vertical sidebar navigation (`@radix-ui/react-tabs` with `orientation="vertical"`) containing 6 items (Profiles, Launch, Install, Browse, Compatibility, Settings), a single-purpose content area per view, and a persistent `ConsoleDrawer` at the bottom. State sharing across the sidebar/content boundary uses two React Contexts (`ProfileContext` wrapping `useProfile()`, `PreferencesContext` wrapping settings/steam paths), eliminating the current prop-drilling through `App.tsx` which holds all top-level state in a 367-line god component.

## Relevant Files

- src/crosshook-native/src/App.tsx: Root shell, all top-level state, tab navigation, heading derivation — primary refactoring target (367 lines -> ~60)
- src/crosshook-native/src/components/ProfileEditor.tsx: Profile sub-tab switching, review modal orchestration — split into ProfilesPage + InstallPage (588 lines)
- src/crosshook-native/src/components/ProfileFormSections.tsx: Shared form renderer, conditional fields by launch method — reused as-is (695 lines)
- src/crosshook-native/src/components/LaunchPanel.tsx: Launch controls with install-context branch to remove, heavy inline styles (257 lines)
- src/crosshook-native/src/components/LauncherExport.tsx: Export with delete/status lifecycle, ~100 lines of inline style constants (655 lines)
- src/crosshook-native/src/components/ConsoleView.tsx: Independent log stream, inline styles despite matching CSS classes existing unused (270 lines)
- src/crosshook-native/src/components/SettingsPanel.tsx: Settings sub-sections with layoutStyles record (470 lines)
- src/crosshook-native/src/components/CommunityBrowser.tsx: Community profiles with panelStyles object (612 lines)
- src/crosshook-native/src/components/CompatibilityViewer.tsx: Compatibility data viewer with inline styles
- src/crosshook-native/src/components/InstallGamePanel.tsx: Guided install wizard flow (547 lines)
- src/crosshook-native/src/components/ProfileReviewModal.tsx: Portal-based modal with focus trap, inert sibling handling (457 lines)
- src/crosshook-native/src/components/AutoPopulate.tsx: Steam auto-discovery with inline styles (320 lines)
- src/crosshook-native/src/hooks/useProfile.ts: Profile CRUD state machine — wrap in ProfileContext (479 lines)
- src/crosshook-native/src/hooks/useLaunchState.ts: Launch phase state machine with reducer pattern (244 lines)
- src/crosshook-native/src/hooks/useInstallGame.ts: Install flow state, prefix resolution, validation
- src/crosshook-native/src/hooks/useGamepadNav.ts: Gamepad polling, focus management — highest risk during restructure (473 lines)
- src/crosshook-native/src/hooks/useCommunityProfiles.ts: Community tap CRUD, sync, import
- src/crosshook-native/src/styles/theme.css: All CSS classes, modal styles, responsive breakpoints (870 lines)
- src/crosshook-native/src/styles/variables.css: CSS custom properties/design tokens (48 lines)
- src/crosshook-native/src/styles/focus.css: Focus/controller navigation styles, unused `.crosshook-controller-prompts` class (108 lines)
- src/crosshook-native/src/types/index.ts: Type re-exports
- src/crosshook-native/src/types/profile.ts: GameProfile type definition
- src/crosshook-native/src/types/launch.ts: LaunchRequest, LaunchPhase, LaunchMethod types
- src/crosshook-native/src/types/launcher.ts: LauncherInfo, LauncherDeleteResult types
- src/crosshook-native/src/types/settings.ts: AppSettingsData, RecentFilesData types
- src/crosshook-native/src/types/install.ts: InstallGameRequest, InstallGameResult types
- src/crosshook-native/src/types/profile-review.ts: ProfileReviewSession type
- src/crosshook-native/src-tauri/tauri.conf.json: Window config (1280x800, dark theme, AppImage target)
- src/crosshook-native/package.json: Frontend dependencies

## Relevant Patterns

**Hook-based state management**: Each domain area has its own hook (`useProfile`, `useLaunchState`, `useInstallGame`, `useCommunityProfiles`, `useGamepadNav`). New views consume the same hooks via React Context. See [src/crosshook-native/src/hooks/useProfile.ts](src/crosshook-native/src/hooks/useProfile.ts) for the primary pattern.

**Tauri IPC via invoke()**: All backend operations use `invoke()` from `@tauri-apps/api/core`. No direct filesystem calls in the frontend. See [src/crosshook-native/src/App.tsx](src/crosshook-native/src/App.tsx) lines 176-182 for the `Promise.all([invoke(...)])` load pattern.

**BEM-like CSS naming**: Components use `crosshook-component`, `crosshook-component--modifier`, `crosshook-component__element` classes in `theme.css`. New sidebar/layout CSS must follow this convention. See [src/crosshook-native/src/styles/theme.css](src/crosshook-native/src/styles/theme.css).

**CSS custom properties**: Design tokens in `variables.css` for colors, spacing, radii, shadows, fonts. All new CSS must reference `--crosshook-*` variables instead of hardcoded values. See [src/crosshook-native/src/styles/variables.css](src/crosshook-native/src/styles/variables.css).

**Modal focus trapping**: `ProfileReviewModal` implements full accessibility — portal to `document.body`, inert siblings via `hiddenNodesRef`, scroll lock, keyboard escape, `data-crosshook-focus-root="modal"` for gamepad hook. New layout must not break this. See [src/crosshook-native/src/components/ProfileReviewModal.tsx](src/crosshook-native/src/components/ProfileReviewModal.tsx).

**Gamepad navigation scope**: `useGamepadNav` attaches to `rootRef` and traverses focusable elements in DOM order. Modal override via `MODAL_FOCUS_ROOT_SELECTOR`. Arrow key events use capture-phase `preventDefault()`. See [src/crosshook-native/src/hooks/useGamepadNav.ts](src/crosshook-native/src/hooks/useGamepadNav.ts).

**Touch target minimum**: `--crosshook-touch-target-min: 48px` ensures gamepad/touch friendliness on all interactive elements.

## Relevant Docs

**docs/plans/ui-enhancements/feature-spec.md**: You _must_ read this when working on any UI enhancement task. Contains all resolved decisions, architecture design, component tree, and success criteria.

**docs/plans/ui-enhancements/research-technical.md**: You _must_ read this when working on component restructuring, state management, or CSS architecture tasks. Contains the detailed proposed component tree and file change specifications.

**docs/plans/ui-enhancements/research-ux.md**: You _must_ read this when working on sidebar navigation, gamepad UX, or responsive layout tasks. Contains competitive analysis of 6 launchers and gamepad navigation patterns.

**docs/plans/ui-enhancements/research-external.md**: You _must_ read this when integrating Radix UI or react-resizable-panels. Contains API documentation, code examples, and CSS integration patterns.

**docs/plans/ui-enhancements/research-business.md**: You _must_ read this when working on user workflows, state flow changes, or component coupling decisions. Contains domain model and component coupling analysis.

**CLAUDE.md**: You _must_ read this for project conventions, commit message format, and build commands.
