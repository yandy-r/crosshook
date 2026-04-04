# Protontricks Integration

CrossHook is a native Linux Tauri v2 app (Rust backend in `crosshook-core`, thin IPC layer in `src-tauri`, React/TypeScript frontend) that orchestrates launching Windows games via Proton/Wine. The protontricks-integration feature adds a new `prefix_deps` module to `crosshook-core` that detects winetricks/protontricks binaries, checks WINE prefix dependencies via `winetricks list-installed`, installs missing packages via streamed subprocess execution, and persists state in a new SQLite `prefix_dependency_state` table (migration v14->v15). It extends the profile TOML schema (`TrainerSection.required_protontricks`), app settings (`protontricks_binary_path`), exposes 4 new Tauri IPC commands, and adds a `PrefixDepsPanel` React component with a dedicated `usePrefixDeps` hook.

## Relevant Files

### Core Library (crosshook-core)

- src/crosshook-native/crates/crosshook-core/src/lib.rs: Module registry; add `pub mod prefix_deps;` here
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: `resolve_umu_run_path()` (PATH walk binary detection pattern at line 301), `apply_host_environment()`, `resolve_wine_prefix_path()` — reuse these
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Subprocess spawn patterns using `tokio::process::Command` — model for winetricks/protontricks invocation
- src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs: Embedded TOML + user-override catalog pattern (OnceLock); potential model for package allowlist
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile`, `TrainerSection`, `RuntimeSection` — add `required_protontricks: Vec<String>` to `TrainerSection`
- src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: `CommunityProfileManifest`; community profiles declare `required_protontricks` via same TOML extension
- src/crosshook-native/crates/crosshook-core/src/profile/health.rs: `ProfileHealthReport`, `HealthIssue`, `HealthStatus` — extend to surface missing-dependency health issues
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `AppSettingsData`, `SettingsStore` — add `protontricks_binary_path` field with `#[serde(default)]`
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential migration runner at schema v14; add `migrate_14_to_15()` for `prefix_dependency_state` table
- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs: SQLite connection factory (WAL, FK enforcement, 0o600 permissions)
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: `MetadataStore` facade with `with_conn()` pattern; add dep-state CRUD methods here
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: Row structs and error types for metadata tables
- src/crosshook-native/crates/crosshook-core/src/install/service.rs: `build_install_command()` pattern — direct Proton command construction, log stdio attachment
- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs: Proton discovery, compat tool mappings, prefix paths under `steamapps/compatdata/<APPID>/pfx`
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: `LaunchRequest`, `RuntimeLaunchConfig.prefix_path` — contains WINE prefix path needed for winetricks

### Tauri IPC Layer (src-tauri)

- src/crosshook-native/src-tauri/src/lib.rs: Tauri builder with `.manage()` state injection and `invoke_handler!` macro; register new commands here
- src/crosshook-native/src-tauri/src/commands/launch.rs: Existing `#[tauri::command]` patterns — async spawn, log streaming via `app.emit()`, `spawn_blocking` for sync I/O
- src/crosshook-native/src-tauri/src/commands/steam.rs: `list_proton_installs` — model for discovery commands returning structured results
- src/crosshook-native/src-tauri/src/commands/settings.rs: TOML settings load/save round-trip pattern
- src/crosshook-native/src-tauri/src/commands/onboarding.rs: System readiness check pattern; add winetricks binary check
- src/crosshook-native/src-tauri/src/commands/update.rs: Cancellable async operation via `Mutex<Option<u32>>` — model for concurrent-install prevention

### Frontend (React/TypeScript)

- src/crosshook-native/src/types/profile.ts: `GameProfile` TypeScript mirror — add `required_protontricks?: string[]`
- src/crosshook-native/src/types/settings.ts: `AppSettingsData` TypeScript mirror — add `protontricks_binary_path`
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Profile detail view; embed `PrefixDepsPanel` in collapsible section
- src/crosshook-native/src/components/SettingsPanel.tsx: Settings UI; add binary path fields for winetricks/protontricks
- src/crosshook-native/src/hooks/useProfile.ts: Main profile state hook; prefix-deps may trigger profile refresh
- src/crosshook-native/src/hooks/useProtonInstalls.ts: Model for new `usePrefixDeps` hook (invoke wrapper, loading/error state, cleanup)
- src/crosshook-native/src/hooks/useLaunchState.ts: Frontend event listener pattern for Tauri events
- src/crosshook-native/src/hooks/useScrollEnhance.ts: SCROLLABLE selector — register any new `overflow-y: auto` container here
- src/crosshook-native/src/styles/variables.css: CSS variables for theming

### Shell / Runtime

- src/crosshook-native/runtime-helpers/steam-launch-helper.sh: WINE env clearing pattern; shows full env var unset list for clean WINE sessions

## Relevant Tables

- profiles: `profile_id TEXT PK`, `game_name`, `launch_method`, `is_favorite` — FK target for new `prefix_dependency_state` table
- health_snapshots: `profile_id PK FK`, `status`, `issue_count`, `checked_at` — extend health checks to include prefix dep status
- prefix_dependency_state (NEW, v14->v15): `profile_id FK`, `package_name`, `prefix_path`, `state` (unknown|installed|missing|install_failed|check_failed), `checked_at`, `installed_at`, `last_error` — tracks per-profile, per-package, per-prefix installation state

## Relevant Patterns

**Thin IPC Command Layer**: Tauri commands in `src-tauri/src/commands/` delegate directly to `crosshook-core`. They do input validation, state extraction from `State<'_>`, and serialize errors to `String` — no business logic. See [src/crosshook-native/src-tauri/src/commands/launch.rs](src/crosshook-native/src-tauri/src/commands/launch.rs) for the canonical example.

**Store Pattern**: Persistent state encapsulated in `*Store` structs (SettingsStore, ProfileStore, MetadataStore) with `load()`/`save()` methods returning typed `Result<T, *StoreError>`. Registered via `.manage()` in `lib.rs`. See [src/crosshook-native/crates/crosshook-core/src/settings/mod.rs](src/crosshook-native/crates/crosshook-core/src/settings/mod.rs).

**Process Execution via tokio::process::Command**: All subprocess launches use `Command::new(exe)` with `.arg()` only (never shell interpolation), `env_clear()` + explicit env vars for Proton commands. **Exception**: protontricks/winetricks need a full POSIX environment (HOME, USER, PATH, XDG_RUNTIME_DIR) — do NOT use `env_clear()` for these; use `apply_host_environment()` instead. See [src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs](src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs).

**Binary Detection via PATH Walk**: `resolve_umu_run_path()` at `runtime_helpers.rs:301` iterates `env::split_paths()` and checks `is_executable_file()`. Reuse this exact pattern for winetricks/protontricks binary detection.

**Tauri Event Streaming**: Long-running operations spawn a background task that reads process output and emits events via `app.emit("event-name", payload)`. Frontend hooks listen via `listen()` from `@tauri-apps/api/event`. See [src/crosshook-native/src-tauri/src/commands/launch.rs:350](src/crosshook-native/src-tauri/src/commands/launch.rs) and [src/crosshook-native/src/hooks/useLaunchState.ts](src/crosshook-native/src/hooks/useLaunchState.ts).

**Type Mirror Pattern**: Rust structs crossing IPC derive `Serialize + Deserialize` with serde. TypeScript interfaces mirror them with `snake_case` field names. See `LaunchRequest` in [src/crosshook-native/crates/crosshook-core/src/launch/request.rs:23](src/crosshook-native/crates/crosshook-core/src/launch/request.rs) and [src/crosshook-native/src/types/launch.ts:23](src/crosshook-native/src/types/launch.ts).

**SQLite Migration Pattern**: Sequential migrations in `metadata/migrations.rs` guarded by `if version < N { migrate... }`. New migration is `migrate_14_to_15()`. See [src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs](src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs).

**React Hook Pattern**: Each feature area has a dedicated `use*.ts` hook wrapping `invoke()`, owning loading/error state with cleanup flag (`let active = true; return () => { active = false; }`). See [src/crosshook-native/src/hooks/useProtonInstalls.ts](src/crosshook-native/src/hooks/useProtonInstalls.ts).

**Concurrent Operation Lock**: `Mutex<Option<u32>>` holding child PID to prevent concurrent operations against the same resource. See [src/crosshook-native/src-tauri/src/commands/update.rs](src/crosshook-native/src-tauri/src/commands/update.rs). Apply to prefix dependency installation.

**Serde Backward Compatibility**: New TOML fields use `#[serde(default, skip_serializing_if = "Vec::is_empty")]` (or `String::is_empty`) so existing files remain valid. See `TrainerSection` in [src/crosshook-native/crates/crosshook-core/src/profile/models.rs:220](src/crosshook-native/crates/crosshook-core/src/profile/models.rs).

## Relevant Docs

**docs/plans/protontricks-integration/feature-spec.md**: You _must_ read this before writing any code. The authoritative resolved spec with architecture diagram, all data models, all 4 IPC command specs, business rules, success criteria, and risk register.

**AGENTS.md**: You _must_ read this when working on any component. Platform rules (CrossHook is NOT a Wine app; it orchestrates Wine), directory map, SQLite schema inventory, persistence classification, IPC naming conventions.

**docs/plans/protontricks-integration/research-security.md**: You _must_ read this before implementing the runner, validation, or IPC layer. 7 CRITICAL findings that block ship: `Command::arg()` (never shell), `--` separator for verbs, no raw subprocess output to UI, per-prefix concurrent lock.

**docs/plans/protontricks-integration/research-practices.md**: You _must_ read this before implementing each phase. Exact reusable file inventory with line numbers — prevents re-implementing existing code (e.g., `resolve_umu_run_path` pattern, `ScopedCommandSearchPath` test pattern, `open_in_memory()` for SQLite tests).

**docs/plans/protontricks-integration/research-technical.md**: You _must_ read this for deep architecture detail. Component diagram, SQLite migration DDL, TOML field additions, Rust type definitions, IPC integration points.

**docs/plans/protontricks-integration/research-recommendations.md**: You _must_ read this for phasing strategy and blocking decisions. Contains the unresolved static vs. dynamic allowlist decision.

**docs/plans/protontricks-integration/research-ux.md**: Read when implementing UI components. `DependencyStatusBadge` design (do NOT extend `HealthBadge`), `DepStatus` type, ConsoleDrawer event integration, concurrent lock enforcement in UI.

**docs/plans/protontricks-integration/research-external.md**: Reference when building the `Command`. CLI reference for winetricks and protontricks: env vars, exit codes, prefix path conventions.

**docs/plans/protontricks-integration/research-business.md**: Reference for behavioral decisions. 14 business rules (BR-1 to BR-14): soft-block vs. hard-block, 24h TTL, Flatpak handling.

**docs/features/steam-proton-trainer-launch.doc.md**: Read when integrating launch gate and prefix panel. How CrossHook handles prefixes, launch flows, ConsoleView usage, health dashboard integration.
