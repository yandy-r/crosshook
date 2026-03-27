# Profile Health Dashboard

The profile health dashboard adds batch filesystem-path validation to CrossHook's profile management, surfacing per-profile health status (healthy/stale/broken) inline on the profile list. The Rust backend (`crosshook-core`) already provides `validate_all()` with structured `ValidationError` → `LaunchValidationIssue` mapping, `ProfileStore::list()`/`load()` for batch iteration, and method-aware validation dispatch — the new `profile/health.rs` module validates `GameProfile` path fields directly via `std::fs::metadata()` without constructing a `LaunchRequest`. The frontend integrates through a new `useProfileHealth` hook calling two new Tauri IPC commands (`batch_validate_profiles`, `get_profile_health`), with `HealthBadge` components reusing the existing `crosshook-status-chip` CSS pattern from `CompatibilityViewer.tsx`.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Contains `validate_all()`, `ValidationError` enum with `.help()` remediation text, `ValidationSeverity`, private path-checking helpers (`require_directory()`, `require_executable_file()`, `is_executable_file()`) that need promotion to `pub(crate)`
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` struct with all path fields (`game.executable_path`, `trainer.path`, `steam.proton_path`, `steam.compatdata_path`, `runtime.prefix_path`, `runtime.proton_path`, `injection.dll_paths`, `steam.launcher.icon_path`)
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore` with `list()`, `load()`, `save()`, `with_base_path()` (critical for testing), `validate_name()` path traversal protection
- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs: Profile module root — needs `pub mod health;` added
- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs: `LauncherInfo.is_stale` field pattern — analogous staleness detection on stored data
- src/crosshook-native/crates/crosshook-core/src/lib.rs: Crate module root — health types exported through `profile::health`
- src/crosshook-native/src-tauri/src/commands/launch.rs: Existing `validate_launch` Tauri command pattern; `sanitize_display_path()` at line ~301 for path privacy
- src/crosshook-native/src-tauri/src/commands/profile.rs: Existing profile CRUD commands — new health commands added here alongside `list_profiles`, `load_profile`, etc.
- src/crosshook-native/src-tauri/src/commands/mod.rs: Command module registry
- src/crosshook-native/src-tauri/src/lib.rs: Tauri setup + `invoke_handler` command registration; startup task spawning pattern at lines ~46-56
- src/crosshook-native/src-tauri/src/startup.rs: Auto-load profile on startup — health check must NOT be added to this synchronous path
- src/crosshook-native/src-tauri/tauri.conf.json: `"csp": null` at line ~23 — security warning W-1 requires CSP enablement
- src/crosshook-native/src/components/LaunchPanel.tsx: Launch validation display with `severityIcon()`, `crosshook-launch-panel__feedback-*` CSS classes, `isStale()` (60s preview staleness — do NOT reuse for health)
- src/crosshook-native/src/components/CompatibilityViewer.tsx: `crosshook-status-chip crosshook-compatibility-badge--{rating}` badge pattern to reuse for health badges
- src/crosshook-native/src/components/ProfileFormSections.tsx: Profile editing form sections — 25k component containing all profile field editors
- src/crosshook-native/src/components/ProfileActions.tsx: Profile action buttons (save, delete, rename, duplicate, export)
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Profile list page — primary integration point for inline health badges
- src/crosshook-native/src/hooks/useLaunchState.ts: `useReducer` + typed actions async state machine pattern — model for `useProfileHealth` hook
- src/crosshook-native/src/hooks/useProfile.ts: Profile CRUD state management — health hook will call revalidation after profile save
- src/crosshook-native/src/types/profile.ts: Existing TypeScript profile types
- src/crosshook-native/src/types/launch.ts: `LaunchFeedback` discriminated union, `LaunchValidationSeverity` — pattern for health types
- src/crosshook-native/src/types/index.ts: Type re-exports — add `export * from './health'`
- src/crosshook-native/src/styles/variables.css: CSS custom properties including `--crosshook-color-success/warning/danger`, `--crosshook-touch-target-min: 48px`
- src/crosshook-native/src/App.tsx: Main app shell with tab routing
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: Expandable section component for health detail panels

## Relevant Patterns

**Tauri IPC Command Pattern**: All backend operations are exposed as `#[tauri::command]` functions accepting `State<ProfileStore>`, returning `Result<T, String>`. Frontend calls via `invoke<T>('command_name', { args })`. See [src/crosshook-native/src-tauri/src/commands/launch.rs] for the `validate_launch` command.

**Validation Issue Pipeline**: `ValidationError` enum variants → `.message()` + `.help()` + `.severity()` → `LaunchValidationIssue` struct with Serde derives. Health issues follow the same pipeline but with `HealthIssueKind` enum for machine-readable classification.

**Status Badge Pattern**: `<span class="crosshook-status-chip crosshook-compatibility-badge--{rating}">` with color tokens from CSS custom properties. See [src/crosshook-native/src/components/CompatibilityViewer.tsx] for the `CompatibilityBadge` implementation.

**Async State Hook Pattern**: `useReducer` with typed action/state unions, async `invoke()` calls dispatching `pending → success | error` transitions. See [src/crosshook-native/src/hooks/useLaunchState.ts] for the `LaunchState` reducer.

**CollapsibleSection Pattern**: `<details>/<summary>` wrapper with consistent styling, used for progressive disclosure of validation details. See [src/crosshook-native/src/components/ui/CollapsibleSection.tsx].

**ProfileStore Test Pattern**: `tempfile::tempdir()` + `ProfileStore::with_base_path(temp_path)` for real filesystem tests without mocking. See test functions in [src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs].

**Path Sanitization Pattern**: `sanitize_display_path()` replaces `$HOME` with `~` for display. Must be applied to all path strings before IPC serialization. See [src/crosshook-native/src-tauri/src/commands/launch.rs].

**Gamepad Navigation**: `useGamepadNav` hook manages D-pad navigation with two-zone model (sidebar + content). All interactive elements need `tabindex >= 0` and `min-height: 48px`. See [src/crosshook-native/src/hooks/useGamepadNav.ts].

## Relevant Docs

**docs/plans/profile-health-dashboard/feature-spec.md**: You _must_ read this for complete business rules, data model definitions (Rust structs + TypeScript interfaces), API design, UX workflows, security findings, and phased task breakdown.

**docs/plans/profile-health-dashboard/research-technical.md**: You _must_ read this when working on Rust data models, Tauri command implementations, or validation logic — contains complete code examples and architecture decisions.

**docs/plans/profile-health-dashboard/research-business.md**: You _must_ read this when working on health classification rules, method-aware validation logic, or notification behavior.

**docs/plans/profile-health-dashboard/research-practices.md**: You _must_ read this when making module boundary decisions or choosing between reusing existing types vs. creating new ones.

**docs/plans/profile-health-dashboard/research-security.md**: You _must_ read this when implementing path checking or IPC serialization — contains severity-leveled security findings (0 critical, 3 warnings, 5 advisories).

**docs/plans/profile-health-dashboard/research-ux.md**: You _must_ read this when implementing frontend components — contains gamepad navigation constraints, competitive analysis, and progressive disclosure patterns.

**docs/research/additional-features/implementation-guide.md**: Reference for feature roadmap context — this feature is Phase 2, Order #4, prerequisite for #49 (diagnostic bundle) and #48 (Proton migration).
