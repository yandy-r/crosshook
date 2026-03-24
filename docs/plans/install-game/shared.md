# Install Game

The install-game feature spans the existing native stack end to end: React UI in `/src/crosshook-native/src/`, Tauri IPC commands in `/src/crosshook-native/src-tauri/src/commands/`, and shared Rust domain logic in `/src/crosshook-native/crates/crosshook-core/src/`. The clean fit is a new `install` domain parallel to `launch`, `profile`, `settings`, and `steam`, with a sibling `Install Game` sub-tab inside the existing profile editor rather than a new top-level screen. The install flow should reuse current Proton discovery, profile persistence, and direct `proton run` environment assembly, but it needs install-specific validation, prefix provisioning, bounded executable discovery, and a final review step before saving a standard `proton_run` profile. The implementation should preserve two existing UX contracts from this codebase: detected paths must fill editable fields rather than locking them, and long-running work should report explicit state and logs without disrupting current gamepad-safe input behavior.

## Relevant Files

- /src/crosshook-native/src/components/ProfileEditor.tsx: Current profile UI and best insertion point for the install sub-tab.
- /src/crosshook-native/src/hooks/useProfile.ts: Existing profile list/load/save state and metadata synchronization.
- /src/crosshook-native/src/App.tsx: Root composition and current handoff between profile state and runtime actions.
- /src/crosshook-native/src/types/profile.ts: Frontend shape of persisted profiles and runtime fields.
- /src/crosshook-native/src/types/launch.ts: Existing typed runtime request/result pattern to mirror for install.
- /src/crosshook-native/src/styles/theme.css: Existing tab-row and panel styling patterns to reuse.
- /src/crosshook-native/src-tauri/src/lib.rs: Tauri command registration and store injection boundary.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: Thin command adapters for profile persistence.
- /src/crosshook-native/src-tauri/src/commands/steam.rs: Existing Proton discovery and Steam path detection commands.
- /src/crosshook-native/src-tauri/src/commands/launch.rs: Async process launch and log streaming pattern to follow.
- /src/crosshook-native/crates/crosshook-core/src/lib.rs: Shared Rust module export surface where new install domain plugs in.
- /src/crosshook-native/crates/crosshook-core/src/profile/models.rs: Canonical `GameProfile` schema used across frontend and backend.
- /src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: TOML-backed profile persistence and name validation.
- /src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Existing validation pattern and runtime request contracts.
- /src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Direct `proton run` command building, runtime env setup, and log wiring.
- /src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: Filesystem-based Proton discovery and compat-tool resolution.
- /src/crosshook-native/crates/crosshook-core/src/settings/recent.rs: Recent-path persistence pattern if installer media recents are added.
- /docs/plans/install-game/feature-spec.md: Settled product and architecture decisions for this feature.

## Relevant Patterns

**Domain-Oriented Rust Modules**: Shared backend logic is grouped by feature domain, so install should live in a new Rust module rather than bloating launch or profile code. Example: [lib.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/lib.rs).

**Thin Tauri Command Adapters**: Tauri command files stay narrow and delegate business logic to shared Rust code, returning `Result<T, String>` to the frontend. Example: [launch.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs).

**Hook-Owned Frontend State**: Components stay mostly declarative while hooks own normalization, persistence, and async side effects. Example: [useProfile.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts).

**Editable Detected-Path Inputs**: Detected installs should accelerate entry without making the text field dead once a selection is made. Example: [ProfileEditor.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx).

**Typed Validation Errors In Rust**: Validation rules live in Rust with explicit error variants and user-facing messages instead of ad hoc frontend checks. Example: [request.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs).

**Inline Rust Domain Tests**: File-backed persistence and runtime construction are tested close to the domain code with `tempfile` and focused unit tests. Example: [script_runner.rs](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs).

## Relevant Docs

**/docs/plans/install-game/feature-spec.md**: You _must_ read this when working on install-game architecture, UX, and settled product decisions.

**/docs/plans/install-game/research-technical.md**: You _must_ read this when creating the new install domain, Tauri command surface, and executable-discovery flow.

**/docs/plans/install-game/research-ux.md**: You _must_ read this when modifying the profile panel or adding install status and review UI.

**/docs/features/steam-proton-trainer-launch.doc.md**: You _must_ read this when extending Proton/profile behavior so the new flow stays consistent with current launch concepts.

**/docs/getting-started/quickstart.md**: You _must_ read this when deciding what the generated profile should look like and how it fits the current user model.

**/README.md**: You _must_ read this when changing any user-facing storage-path or launch-mode documentation.

**/AGENTS.md**: You _must_ read this when implementing new Rust, Tauri, or React files so repo-specific boundaries and conventions stay consistent.

**/CLAUDE.md**: You _must_ read this when wiring frontend/backend integration because it captures the intended native architecture clearly.

**/tasks/lessons.md**: You _must_ read this when implementing the new sub-tab UI, especially the lessons about editable detected-path fields and gamepad-safe typing.
