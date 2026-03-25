# Profile Modal

The `profile-modal` feature sits at the seam between the install flow and the normal profile editor inside the native React/Tauri frontend. `InstallGamePanel` and `useInstallGame` already produce a reviewable `GameProfile`, executable candidates, and install status, while `ProfileEditorView` currently consumes that data by hydrating global profile state and switching to the Profile tab. The new implementation should keep install execution where it is, move review-session ownership into `ProfileEditorView`, reuse shared profile field sections inside a real portal-based modal, and save through the existing `useProfile` persistence path before explicitly switching to the Profile tab. The main cross-cutting constraints are focus/controller behavior from `useGamepadNav`, viewport-safe modal sizing at 1280x800, and keeping type ownership split between install transport data and modal-local UI state.

## Relevant Files

- /docs/plans/profile-modal/feature-spec.md: Agreed feature contract, decisions, affected files, and implementation phases.
- /src/crosshook-native/src/components/ProfileEditor.tsx: Current install/profile bridge and future modal-session owner.
- /src/crosshook-native/src/components/InstallGamePanel.tsx: Install form, review preview, candidate list, and verify handoff.
- /src/crosshook-native/src/hooks/useInstallGame.ts: Install state machine, review profile derivation, and candidate generation.
- /src/crosshook-native/src/hooks/useProfile.ts: Profile normalization, save validation, persistence, and metadata sync.
- /src/crosshook-native/src/App.tsx: App-level tab state and `useGamepadNav` mounting point.
- /src/crosshook-native/src/hooks/useGamepadNav.ts: Keyboard/controller focus rules that modal behavior must respect.
- /src/crosshook-native/src/types/install.ts: Existing install transport types; home for `InstallProfileReviewPayload`.
- /src/crosshook-native/src/types/profile.ts: Shared `GameProfile` contract used by install, editor, and persistence.
- /src/crosshook-native/src/styles/theme.css: Shared surface, layout, and responsive styling system to extend.
- /src/crosshook-native/src/styles/focus.css: Existing focus styling for keyboard/controller-visible targets.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Tauri profile command boundary reused by modal save flow.
- /src/crosshook-native/src-tauri/src/commands/install.rs: Tauri install command boundary used by `useInstallGame`.
- /src/crosshook-native/crates/crosshook-core/src/install/service.rs: Backend install execution and reviewable-profile generation.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Canonical Rust `GameProfile` model backing TOML persistence.

## Relevant Tables

- None: this feature reuses filesystem-backed TOML persistence and does not involve a database.

## Relevant Patterns

**Hook-Owned Domain State**: Stateful workflows live in hooks and components mostly render derived state plus callbacks. See [useInstallGame.ts](/src/crosshook-native/src/hooks/useInstallGame.ts) and [useProfile.ts](/src/crosshook-native/src/hooks/useProfile.ts).

**Parent-Owned Coordination**: Cross-feature handoff is coordinated in a parent component through callbacks and lifted state, not a global store. See [ProfileEditor.tsx](/src/crosshook-native/src/components/ProfileEditor.tsx) and [InstallGamePanel.tsx](/src/crosshook-native/src/components/InstallGamePanel.tsx).

**Derived State Over Duplicate Truth**: Labels, stages, preview objects, and normalized values are derived through small helpers near the owning hook/component. See [useInstallGame.ts](/src/crosshook-native/src/hooks/useInstallGame.ts) and [useProfile.ts](/src/crosshook-native/src/hooks/useProfile.ts).

**Normalize At Save/Edit Boundaries**: Profiles are normalized entering edit state and again before persistence, rather than relying on UI fallbacks. See [useProfile.ts](/src/crosshook-native/src/hooks/useProfile.ts).

**Shared Theme System**: New UI should prefer `crosshook-*` classes and CSS variables instead of expanding inline-style-only surfaces. See [theme.css](/src/crosshook-native/src/styles/theme.css) and [variables.css](/src/crosshook-native/src/styles/variables.css).

**Type Ownership By Domain**: Transport contracts stay with their domain type file, while UI-only state gets its own module under `src/types`. See [install.ts](/src/crosshook-native/src/types/install.ts), [profile.ts](/src/crosshook-native/src/types/profile.ts), and [index.ts](/src/crosshook-native/src/types/index.ts).

## Relevant Docs

**/docs/plans/profile-modal/feature-spec.md**: You _must_ read this when working on modal ownership, save behavior, collapsed sections, and type placement.

**/docs/plans/profile-modal/research-technical.md**: You _must_ read this when working on component boundaries, modal-local draft ownership, and `useProfile` reuse.

**/docs/plans/profile-modal/research-ux.md**: You _must_ read this when working on viewport sizing, sticky chrome, focus behavior, and internal scrolling.

**/docs/plans/profile-modal/research-external.md**: You _must_ read this when working on portal/dialog primitives, ARIA expectations, and inert background behavior.

**/tasks/lessons.md**: You _must_ read this when working on controller input handling and Tauri dialog capability assumptions.

**/src/crosshook-native/src/components/LaunchPanel.tsx**: You _must_ read this when updating user-facing copy that currently references reviewing in the Profile tab.

**/src/crosshook-native/src/components/LauncherExport.tsx**: You _must_ read this when updating user-facing copy around the install review/save boundary.
