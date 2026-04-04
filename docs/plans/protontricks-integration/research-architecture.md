# Architecture Research: protontricks-integration

## System Overview

CrossHook is a native Linux desktop application built with Tauri v2 (Rust backend + React/TypeScript frontend). Business logic lives in `crosshook-core` (a standalone Rust crate); `src-tauri` provides thin Tauri IPC command wrappers; and the frontend consumes those commands through typed `invoke()` calls wrapped in React hooks. The protontricks-integration feature adds a `prefix_deps` module to `crosshook-core`, a SQLite migration (v14 → v15), TOML profile schema extensions, 4 new Tauri IPC commands, and a new React panel in the Profiles or Launch tab.

## Relevant Components

- `src/crosshook-native/crates/crosshook-core/src/lib.rs`: Module registry for the core library; new `prefix_deps` module must be declared here
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: Contains `resolve_umu_run_path()` (PATH walk pattern to reuse for binary discovery), proton path resolution, and subprocess environment setup
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: Subprocess spawn patterns using `tokio::process::Command` — the exact pattern for winetricks/protontricks invocation
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile`, `RuntimeSection`, `LaunchSection` — `required_protontricks` field goes into a new or existing profile section
- `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`: `CommunityProfileManifest` wraps `GameProfile`; community profiles declare `required_protontricks` via the same profile TOML extension
- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`: `ProfileHealthReport`, `HealthIssue`, `HealthStatus` — extend to surface missing-dependency health issues
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `AppSettingsData` — add `protontricks_binary_path` optional field here
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Sequential migration runner at schema v14; v15 migration adds `prefix_dependency_state` table
- `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs`: SQLite connection factory (WAL, FK enforcement, 0o600 permissions) — used unchanged by new store
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` facade; new dependency-state methods attach here
- `src/crosshook-native/src-tauri/src/lib.rs`: Tauri builder with `.manage()` state injection and `invoke_handler!` macro; new commands registered here
- `src/crosshook-native/src-tauri/src/commands/launch.rs`: Existing `#[tauri::command]` pattern to follow for new prefix-deps commands
- `src/crosshook-native/src/types/profile.ts`: `GameProfile` TypeScript mirror — needs `required_protontricks?: string[]` field
- `src/crosshook-native/src/types/settings.ts`: `AppSettingsData` TypeScript mirror — needs dependency settings fields
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`: Profile detail view; `PrefixDepsPanel` will be embedded here under a collapsible section
- `src/crosshook-native/src/components/SettingsPanel.tsx`: Settings UI; binary path fields for winetricks/protontricks go here
- `src/crosshook-native/src/hooks/useProfile.ts`: Main profile state hook; prefix-deps interaction may trigger profile refresh or status update

## Data Flow

**Dependency check flow:**

1. Frontend calls `invoke('check_prefix_deps', { profile_name, prefix_path })` → `src-tauri/commands/prefix_deps.rs` → `crosshook_core::prefix_deps::check(prefix_path, verbs)` → spawns `WINEPREFIX=<path> winetricks list-installed` as `tokio::process::Command` (env_clear + apply_host_environment pattern) → parses stdout → returns `Vec<DepStatus>`.
2. Results cached in SQLite `prefix_dependency_state` table via `MetadataStore`; TTL 24 hours per `feature-spec.md` BR-5.

**Dependency install flow:**

1. Frontend calls `invoke('install_prefix_deps', { profile_name, prefix_path, verbs })` → Tauri command spawns `WINEPREFIX=<path> winetricks -q <verbs>` (or `protontricks <appid> -q <verbs>` if configured), streams stdout/stderr back via Tauri `emit()` to the console drawer (same pattern as `launch_game`).
2. On completion, SQLite cache is invalidated; frontend re-checks status.

**Settings persistence:**

- `winetricks_path` and `protontricks_path` stored in `~/.config/crosshook/settings.toml` via `AppSettingsData` (TOML store, not SQLite).

**Profile TOML extension:**

- `required_protontricks = ["vcrun2019", "dotnet48"]` added to `GameProfile` as a new field (top-level or under `[trainer]` section); Serde `default` + `skip_serializing_if` for backward compatibility.

## Integration Points

| Touch Point                                 | What Changes                                                                                                                                  |
| ------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `crosshook-core/src/lib.rs`                 | Add `pub mod prefix_deps;`                                                                                                                    |
| `crosshook-core/src/profile/models.rs`      | Add `required_protontricks: Vec<String>` to `GameProfile` or `TrainerSection` with `#[serde(default, skip_serializing_if = "Vec::is_empty")]` |
| `crosshook-core/src/settings/mod.rs`        | Add `winetricks_path: String` / `protontricks_path: String` to `AppSettingsData` with `#[serde(default)]`                                     |
| `crosshook-core/src/profile/health.rs`      | Extend `check_profile_health()` to emit amber `HealthIssue` when declared deps are unchecked or missing                                       |
| `crosshook-core/src/metadata/migrations.rs` | Add `migrate_14_to_15()` for `prefix_dependency_state` table; increment guard to `< 15`                                                       |
| `crosshook-core/src/metadata/mod.rs`        | Add `check_dep_cache()`, `upsert_dep_cache()`, `invalidate_dep_cache()` methods to `MetadataStore`                                            |
| `src-tauri/src/commands/`                   | New `prefix_deps.rs` file with `check_prefix_deps`, `install_prefix_deps`, `detect_winetricks_binary`, `get_prefix_dep_status` commands       |
| `src-tauri/src/lib.rs`                      | Register new commands in `invoke_handler!` macro                                                                                              |
| `src/types/profile.ts`                      | Add `required_protontricks?: string[]`                                                                                                        |
| `src/types/settings.ts`                     | Add `winetricks_path: string` / `protontricks_path: string`                                                                                   |
| `src/components/`                           | New `PrefixDepsPanel.tsx`; add to `ProfilesPage.tsx` inside a collapsible section                                                             |
| `src/components/SettingsPanel.tsx`          | Add binary path fields                                                                                                                        |
| `src/hooks/useScrollEnhance.ts`             | Register `PrefixDepsPanel` scroll container selector if it uses `overflow-y: auto`                                                            |

## Key Dependencies

**Internal (all already in Cargo.toml):**

- `tokio::process::Command` — async subprocess with `env_clear()` (already used in `script_runner.rs`)
- `rusqlite` (bundled) — SQLite state persistence via existing `MetadataStore` infrastructure
- `serde` / `toml` — TOML profile serialization; new fields are backward-compatible with `#[serde(default)]`
- `tracing` — structured log events (existing pattern throughout `crosshook-core`)
- `directories::BaseDirs` — resolves `~/.config/crosshook/` and `~/.cache/` paths

**External binaries (runtime, not compile-time):**

- `winetricks` — primary tool; detected via PATH walk matching `resolve_umu_run_path()` in `launch/runtime_helpers.rs:302`
- `protontricks` — secondary, user-configured; also supports flatpak variant (`flatpak run com.github.Matoking.protontricks`)

**No new crates are required.** The feature-spec explicitly confirms this.

**Architecture constraints to respect:**

- Business logic entirely in `crosshook-core`, not in `src-tauri` commands (CLAUDE.md: "Keep `src-tauri` thin — IPC only")
- All subprocess invocations use `Command::arg()` (not shell interpolation) to prevent command injection
- New `prefix_deps` module follows the same directory-with-`mod.rs` pattern as `install/`, `launch/`, `profile/`
- Scroll containers added to `SCROLLABLE` selector in `src/hooks/useScrollEnhance.ts` (CLAUDE.md: scroll jank rule)
- SQLite migration must be sequential; new migration is `migrate_14_to_15`, guarded by `if version < 15`
