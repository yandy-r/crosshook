# Protontricks Integration — Business Analysis

## Executive Summary

Gamers running Windows trainers via CrossHook routinely hit silent launch failures because trainer executables depend on runtime libraries (VC++ redistributables, .NET, DirectX) that are absent from a fresh WINE prefix. Today users must manually look up the right `protontricks <appid> <package>` invocations, often with no guidance from the app. Adding `required_protontricks` to the community profile schema lets profile authors declare those dependencies once; CrossHook then detects missing packages and can install them automatically or on user request. The feature turns an opaque failure mode into a solvable, guided workflow.

---

## User Stories

### Primary Actor: Gamer Running Trainers via CrossHook

**US-1 — Guided first-time setup**
As a gamer opening a profile for the first time, I want CrossHook to tell me which prefix dependencies are missing and offer to install them, so I don't have to figure out protontricks commands myself.

**US-2 — Community profile applies dependencies**
As a gamer importing a community profile, I want any declared `required_protontricks` packages to be automatically checked (and installed if missing) before the first launch, so the game and trainer work out of the box.

**US-3 — Manual dependency management**
As a power user, I want to open a "Prefix Dependencies" panel for any profile and manually trigger installs, view installed packages, and add packages beyond what the profile declares, so I have full control without leaving the app.

**US-4 — Dependency health at a glance**
As a gamer with many profiles, I want the profile list and health check to flag profiles whose prefix is missing declared dependencies, so I know which profiles need attention before trying to launch.

**US-5 — Profile author declaring dependencies**
As a community profile author, I want to add `required_protontricks = ["vcrun2019", "dotnet48"]` to my profile TOML and have CrossHook honour that list, so other users don't have to reverse-engineer what my trainer needs.

**US-6 — Settings: protontricks binary path**
As a user with a non-standard protontricks installation (e.g. Flatpak), I want to configure the path to the protontricks/winetricks binary in Settings, so CrossHook can find it regardless of where it is installed.

---

## Business Rules

### Core Rules

**BR-1: Package name allowlist**
Package names must match a predefined static allowlist of safe winetricks verbs. Unknown names are rejected at profile load and at runtime with a clear error message. This prevents arbitrary shell injection via community profiles. The allowlist is compiled into CrossHook (not derived at runtime) for security and to avoid requiring a binary at startup.

Confirmed allowlisted packages (verified available in winetricks):
- `vcrun2019` — Visual C++ 2015-2019 redistributable (~50 MB, 2-5 min)
- `vcrun2022` — Visual C++ 2022 redistributable
- `dotnet48` — .NET Framework 4.8 (installs 4.0–4.8; up to 500 MB, 10-20 min)
- `dotnet6` — .NET 6 runtime
- `dotnet7` — .NET 7 runtime
- `d3dx9` — DirectX 9 DLLs
- `d3dcompiler_47` — DirectX shader compiler
- `dxvk` — DXVK Vulkan-based D3D implementation
- `corefonts` — Core Microsoft fonts
- `xact` — Microsoft XACT audio runtime
- `xna40` — XNA Framework 4.0
- `physx` — NVIDIA PhysX

The full winetricks verb list is at https://github.com/Winetricks/winetricks/blob/master/files/verbs/all.txt. Profile authors wanting an unlisted package must request it be added to the allowlist via the CrossHook issue tracker.

**BR-2: Preferred binary is winetricks; protontricks is secondary**
winetricks alone (invoked with `WINEPREFIX` set) is the preferred binary because it does not require Steam to be running and works for all CrossHook prefix configurations. Discovery order:
1. User-configured path in `settings.toml` (`protontricks_binary_path`)
2. `winetricks` on `$PATH`
3. `protontricks` on `$PATH` (used only when Steam App ID is present and Steam is running)

This inverts the earlier preference: winetricks is now the default, with protontricks as an opt-in for users who have it and a Steam App ID configured. Binary discovery is cached for the session; a re-check is triggered when the user saves a new path in Settings.

**BR-3: Prefix path required**
Dependency checks and installs require a non-empty `runtime.prefix_path` in the profile. Profiles without a configured prefix show a "prefix not configured" warning on the dependency panel; install operations are disabled.

**BR-4: winetricks is the standard invocation; protontricks requires Steam running**
CrossHook invokes `WINEPREFIX=<prefix_path> winetricks <packages>` as the standard path. When the user explicitly has protontricks configured AND `steam.app_id` is present AND Steam is detected as running, CrossHook may use `protontricks <appid> <packages>` instead. If Steam is not running, the protontricks invocation is skipped and the winetricks path is used. The dependency panel indicates which invocation mode is active. This rule replaces the earlier "protontricks preferred" framing.

**BR-5: Dependency check uses `winetricks.log` as primary source**
winetricks tracks installed verbs in `$WINEPREFIX/winetricks.log`. CrossHook reads this file as the primary detection mechanism. A package is considered `installed` if its verb name appears in `winetricks.log`. A package is `missing` if the file exists but does not contain the verb. A package is `unknown` if `winetricks.log` does not exist (prefix not yet touched by winetricks) or if the log entry cannot be parsed. This is faster, more reliable, and more idiomatic than registry probing. Check results cached in SQLite are considered fresh for **24 hours**; CrossHook does NOT re-verify on every launch.

**BR-6: Missing dependency is a soft-block, not a hard failure**
An unmet declared dependency is a **warning** (amber), not a launch-blocking error (red), unless `auto_install_prefix_deps` is enabled. The launch proceeds after the user dismisses the prompt or explicitly skips. This maps to `HealthStatus::Stale` rather than `HealthStatus::Broken` in the existing health model. The rationale: the user may have installed the dependency through another mechanism (e.g. manually via winecfg, or through Steam's own prefix management) that CrossHook cannot detect. Hard-blocking would create false refusals.

**BR-7: Auto-install on first launch vs. prompt**
When `required_protontricks` is non-empty and packages are `missing` or `unchecked`:
- If `auto_install_prefix_deps` is enabled in settings (default: `false`): install silently before launching, with a visible "Installing dependencies..." progress indicator.
- Otherwise: surface a "Missing dependencies" banner with [Install] / [Skip] buttons.

Skipping a prompt marks the packages as `user_skipped` in SQLite. The prompt does not recur on subsequent launches for those packages unless the user requests a fresh check or the TTL expires and a re-check finds them missing again. The health indicator remains amber while `user_skipped`.

**BR-8: One active install at a time per prefix path**
Only one dependency install operation may run at a time for a given prefix path. Attempting a second install while one is in progress (including a concurrent game launch that triggers auto-install) is rejected with a clear "Installation already in progress for this prefix" error. Multiple prefixes may have concurrent installs.

**BR-9: Concurrent launch-while-installing is blocked at UI level**
When a dependency install is running for a profile, the launch button for that profile is disabled. If two profiles share the same prefix and one has an install running, the other profile's launch button is also disabled until the install completes. This is enforced by the frontend using the per-prefix active install state (runtime-only).

**BR-10: Installation is atomic per package list**
All packages in a single install request are passed to winetricks in one invocation. If the invocation fails, all packages in that batch are marked `install_failed`. The error output is captured and stored in `install_error` in SQLite for display in the dependency panel. Re-running an already-installed verb is safe (winetricks skips it by default) so retrying a failed batch that partially succeeded is not destructive.

**BR-11: Dependency state persists across restarts**
Check results and install outcomes are persisted in SQLite (`prefix_dependency_states` table, new). The last-checked timestamp and install status survive app restarts and are surfaced in the UI without re-checking every time.

**BR-12: Active installation progress is runtime-only**
While an install is running, live progress (stdout/stderr stream) is held in memory only and emitted to the frontend via Tauri events. It is not persisted in SQLite.

**BR-13: Community profile schema version bump**
Adding `required_protontricks` to `CommunityProfileManifest` bumps `COMMUNITY_PROFILE_SCHEMA_VERSION` to 2. Old clients (schema v1) silently ignore the new field because TOML unknown fields are dropped on deserialization; new clients treat a missing field as an empty list. Users on old CrossHook versions simply won't have dependencies installed — the profile still loads and launches. The upgrade path note is included in the changelog.

**BR-14: User-added packages beyond profile requirements**
Users may add extra packages through the manual dependency panel. These are stored in a `user_extra_protontricks` field in the profile TOML (user-editable, not shared via community). They are checked and installed alongside the declared `required_protontricks`.

**BR-15: Flatpak protontricks is supported but requires explicit configuration**
Flatpak-packaged protontricks (`com.github.Matoking.protontricks`) is supported at launch as a first-class path. Because Flatpak cannot be auto-detected reliably, users must configure `protontricks_binary_path` in Settings. The Settings screen displays a help note with the exact string to paste: `flatpak run com.github.Matoking.protontricks`. Steam Deck users running Flatpak Steam are explicitly called out in the Settings tooltip. There is no auto-detection of Flatpak variants. Note: the Flatpak variant of winetricks does not have this complexity; native winetricks is preferred.

**BR-16: Community trust disclosure including network download warning**
When a community profile is imported with a non-empty `required_protontricks` list, the import preview must include a notice: "This profile will install the following packages into your WINE prefix: [list]. Installing these packages requires an internet connection and downloads files from Microsoft and other sources. Only import profiles from sources you trust." The user must acknowledge this before completing the import (checkbox or [Confirm] button in the preview). This is a one-time acknowledgment per import, not per launch.

Package-specific size and time estimates drawn from the allowlist metadata are shown alongside each package name (e.g. "dotnet48 — up to 500 MB, ~15 min").

**BR-17: Network download is user-visible and user-initiated**
winetricks verbs download from Microsoft CDNs and other sources during installation. CrossHook must make this visible before the first install runs. The trust disclosure (BR-16) and the dependency panel both note that installation requires an active internet connection and may download significant amounts of data. CrossHook does not itself make any network requests for this feature; all downloads are handled by winetricks internally.

**BR-18: Steam must be running when using protontricks by App ID**
If CrossHook resolves to using `protontricks <appid> <packages>` (i.e. protontricks binary found, steam.app_id present), it must first verify that Steam is running. If Steam is not running, CrossHook falls back to the winetricks path instead of failing. This check is done at install time, not at profile load time.

### Edge Cases

**EC-1: Prefix not yet initialized**
If the prefix directory exists but has never been populated by Proton (no `pfx/` subdirectory), dependency checks report "prefix not initialized" and install is blocked with a suggestion to launch the game once first to let Proton initialize the prefix. `winetricks.log` will not exist in this state.

**EC-2: Same package declared multiple times**
Duplicate package names in `required_protontricks` are deduplicated before running checks or installs. No error is raised; duplicates are silently collapsed.

**EC-3: Package installed via external mechanism (not in winetricks.log)**
If the user installed a package via winecfg, Steam, or another tool that does not write to `winetricks.log`, CrossHook will report it as `missing` (log-based check) rather than `installed`. The package is reported as `unknown` only when `winetricks.log` does not exist at all. When the user re-installs the package via CrossHook, winetricks will write to the log and subsequent checks will report `installed`. Re-running an installed verb is safe (winetricks is idempotent).

**EC-4: protontricks requires interactive terminal / GUI**
Some verbs open Wine's own dialogs. CrossHook must run installs with the host `DISPLAY` set and not pipe stdin so Wine dialog boxes can appear. If `DISPLAY` is unset, the install is blocked with a diagnostic.

**EC-5: Profile imported without prefix configured**
If a community profile with `required_protontricks` is imported and the user has not yet set a prefix path, the dependency panel displays the declared dependencies as "cannot check — prefix not configured" and all install buttons are disabled until the prefix is set.

**EC-6: Offline mode / no internet during install**
CrossHook's offline mode flag does not restrict dependency install (the feature works against the local prefix). However, winetricks itself requires an internet connection to download packages. If the internet is unavailable when winetricks runs, it will fail with an error that is captured and shown in the dependency panel. No special CrossHook handling beyond surfacing the error output is required.

**EC-7: Package install fails mid-list**
If `winetricks` exits non-zero, all packages in the batch are marked `install_failed`. The error output is captured and shown in the dependency panel. The user can retry. Because winetricks is idempotent, retrying a partially-completed batch is safe.

**EC-8: Two profiles share the same prefix path**
Dependency states are per-profile, not per-prefix. One profile's install completing does not automatically update the other profile's SQLite state. Each profile independently checks and installs its own declared packages. The UI warns the user when a profile's prefix path is shared with another profile, since installing packages for one profile may affect the other.

**EC-9: dotnet48 installs take 10-20 minutes**
Long-running installs must not appear hung. The UI must show continuous progress (streaming stdout from winetricks) throughout. If the CrossHook window is closed during an install, the winetricks process continues running (it is a child process of the Tauri backend, not the frontend). On next open, CrossHook detects no active install lock (it was runtime-only) and the prefix dependency state shows `unchecked` or `install_failed` depending on whether the process had already written results before close.

---

## Workflows

### Primary Workflow: First-Time Dependency Setup

```
1. User opens a profile (new or imported) with required_protontricks declared.
2. CrossHook loads the profile and reads required_protontricks from [dependencies] section.
3. CrossHook checks prefix_dependency_states in SQLite for cached check results.
   a. If all declared packages have a recent check result (< 24h): use cached state.
   b. Otherwise: schedule a background check.
4. Background check:
   a. Locate winetricks binary (settings → PATH → protontricks fallback if Steam running).
   b. For each package not recently checked (or TTL expired):
      - Read $WINEPREFIX/winetricks.log.
      - If verb appears in log: installed.
      - If log exists but verb absent: missing.
      - If log does not exist: unknown (prefix not yet touched by winetricks).
      - Write result to SQLite with checked_at.
5. If any packages are missing:
   a. If auto_install_prefix_deps = true: proceed to install flow (step 6).
   b. Else: surface "Missing dependencies" banner with [Install] / [Skip] buttons.
      Banner includes estimated download size for missing packages.
6. User clicks Install (or auto-install fires):
   a. Validate: binary path OK, prefix path OK, prefix initialized, DISPLAY set.
   b. If using protontricks path: verify Steam is running (BR-18); fall back to winetricks if not.
   c. Acquire per-prefix install lock (reject if already locked — see BR-8).
   d. Emit install_started event to frontend.
   e. Spawn winetricks (or protontricks) with all missing packages as separate arguments.
   f. Stream stdout/stderr to frontend via Tauri events (runtime-only state).
   g. On success: release lock, re-read winetricks.log, update SQLite to installed, emit install_completed.
   h. On failure: release lock, update SQLite to install_failed, emit install_failed with error output.
7. User proceeds to launch (or CrossHook auto-launches if triggered from launch path).
```

### Workflow: Profile with Declared Dependencies (Community Import)

```
1. User imports a community profile via community tap or file drag-drop.
2. CrossHook reads required_protontricks from the manifest.
3. Import preview (CommunityImportPreview) includes:
   - "required_prefix_deps" list of package names with size/time estimates.
   - Trust disclosure notice with network download warning (BR-16).
   - [Confirm import] button (acknowledges the trust disclosure).
4. After import, if the prefix is configured, step 4 of First-Time Dependency Setup fires.
5. If prefix is not yet configured, a "configure prefix to enable dependency management" notice appears.
```

### Workflow: Manual Dependency Installation

```
1. User opens the "Prefix Dependencies" panel for any profile.
2. Panel shows:
   - Declared required_protontricks (from profile TOML, read-only).
   - User-added extra packages (from local profile TOML field, editable).
   - Last-checked timestamp and current status of each package.
   - Download size estimate for missing/unknown packages.
   - Warning if prefix is shared with another profile.
3. User can:
   a. Click [Check Now] — re-reads winetricks.log, ignores TTL, updates SQLite.
   b. Click [Install Missing] — installs only missing packages.
   c. Click [Install All] — force-reinstalls all packages (winetricks -f flag).
   d. Type a package name and click [Add Package] — validates against allowlist, adds to user_extra_protontricks.
   e. Click [Remove] on a user-added package — removes it from TOML.
4. Progress appears inline in the panel (streamed from runtime state).
5. Results are persisted to SQLite after install completes.
```

### Workflow: Dependency Health Check (Profile List)

```
1. On app start (or manual refresh), CrossHook runs health checks for all profiles.
2. For each profile with required_protontricks + a configured prefix:
   a. Read last check result from SQLite.
   b. If check is stale (> 24 hours) or never run: queue a background check (reads winetricks.log).
3. Health status for each profile includes a "deps_ok | deps_missing | deps_unknown | deps_unchecked" field.
   - deps_missing and deps_unknown map to HealthStatus::Stale (amber, not red).
4. Profile list displays a dependency health indicator (amber dot if missing/unknown).
5. Clicking the indicator navigates to the Prefix Dependencies panel.
```

### Error Recovery Flows

**Binary not found:**
User is directed to Settings > Protontricks. A "Locate binary" file picker allows selecting the executable. The Settings screen shows the Flatpak invocation string as a help note. On save, CrossHook re-checks whether the binary is usable.

**Prefix not initialized:**
CrossHook shows "Run a launch first to initialize the prefix, then retry dependency installation."

**Install failed (network error or winetricks failure):**
Error output is shown inline. User can copy the output, retry, or open a terminal to debug manually. CrossHook logs the failure to the standard log path for the operation. Because winetricks is idempotent, retrying is always safe.

**Concurrent install blocked:**
User sees "Installation already in progress for this prefix. Wait for it to complete before launching." Launch button remains disabled for all profiles sharing that prefix path.

**Steam not running (protontricks path):**
CrossHook automatically falls back to the winetricks path with `WINEPREFIX` set. No user action required. The dependency panel notes the fallback.

---

## Domain Model

### Key Entities

| Entity | Storage | Description |
|---|---|---|
| `GameProfile` | TOML file | The per-game launch configuration. Gains `[dependencies]` section. |
| `required_protontricks` | TOML profile `[dependencies]` | Declared list of winetricks verb names; community-shareable. |
| `user_extra_protontricks` | TOML profile `[dependencies]` | User-added packages; not shared via community profiles. |
| `protontricks_binary_path` | `settings.toml` | Path to winetricks or protontricks binary; user-editable in Settings. |
| `auto_install_prefix_deps` | `settings.toml` | Global toggle for auto-install before launch (default: false). |
| `PrefixDependencyState` | SQLite `prefix_dependency_states` | Per-profile, per-package check result and install outcome. |
| `ActiveInstallProgress` | Runtime memory only | Live install status streamed to the frontend; not persisted. |
| Per-prefix install lock | Runtime memory only | In-memory mutex keyed by prefix path; enforces BR-8. |

### Profile TOML Schema Addition

```toml
[dependencies]
# Packages required by this trainer/game configuration (winetricks verbs).
# Checked and installed automatically via winetricks/protontricks.
required_protontricks = ["vcrun2019", "dotnet48", "d3dx9"]

# User-added packages specific to this machine; not shared with community.
user_extra_protontricks = []
```

### SQLite Entity: `prefix_dependency_states`

Keyed by `(profile_id, package_name)`. Tracks:
- `status`: `unchecked | checking | installed | missing | unknown | install_failed | user_skipped`
- `checked_at`: ISO timestamp of last check (NULL = never checked)
- `installed_at`: ISO timestamp of last successful install (nullable)
- `install_error`: last error message if `install_failed` (nullable)
- `source`: `declared | user_added` — whether the package came from the profile or the user

### Dependency Lifecycle State Machine

```
unchecked
    → checking  (check job dispatched — reads winetricks.log)
        → installed   (verb found in winetricks.log)
        → missing     (log exists, verb not present)
        → unknown     (winetricks.log does not exist)
missing / unknown
    → installing  (install job dispatched — runtime state only)
        → installed       (winetricks exits 0; verb now in winetricks.log)
        → install_failed  (winetricks exits non-zero)
missing / unknown
    → user_skipped  (user dismissed prompt)
user_skipped
    → unchecked     (TTL expired, next health pass)
    → missing       (user requests re-check via [Check Now])
install_failed
    → installing    (user retries — safe, winetricks is idempotent)
installed
    → unchecked     (TTL expired, next health pass)
```

### Relationships

- One `GameProfile` has zero or one `[dependencies]` section.
- One `GameProfile` has zero or many `PrefixDependencyState` rows (one per package name).
- Many `GameProfile`s may share the same prefix path; dependency states are per-profile, not per-prefix, because different games may need different packages in the same prefix.
- `ActiveInstallProgress` is a transient runtime object keyed by `profile_id`.
- The per-prefix install lock is keyed by the resolved absolute prefix path (not profile ID), because the constraint is about concurrent writes to the same WINE prefix on disk.

---

## Existing Codebase Integration

### Profile Schema (`crosshook-core/src/profile/community_schema.rs`)

`CommunityProfileManifest` currently holds `metadata: CommunityProfileMetadata` and `profile: GameProfile`. The `required_protontricks` field lives in `GameProfile.dependencies.required_protontricks` (not in `CommunityProfileMetadata`). `user_extra_protontricks` is also in `GameProfile.dependencies` and is not exported in community manifests (it is a local override).

`COMMUNITY_PROFILE_SCHEMA_VERSION` must increment from 1 to 2. The deserialization must default the field to `Vec::new()` so schema-v1 imports remain valid. Old CrossHook versions (schema v1) will silently drop the field on import — users on those versions won't have dependencies managed. This is documented in the upgrade changelog.

### GameProfile Model (`crosshook-core/src/profile/models.rs`)

A new `DependenciesSection` struct (analogous to `RuntimeSection`, `LaunchSection`) is added to `GameProfile`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DependenciesSection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_protontricks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_extra_protontricks: Vec<String>,
}
```

`GameProfile` gains:
```rust
#[serde(default, skip_serializing_if = "DependenciesSection::is_empty")]
pub dependencies: DependenciesSection,
```

### Settings (`crosshook-core/src/settings/mod.rs`)

`AppSettingsData` gains two new fields:
- `protontricks_binary_path: String` (default: empty, meaning "auto-discover winetricks")
- `auto_install_prefix_deps: bool` (default: false)

The IPC DTO in `src-tauri/src/commands/settings.rs` (`AppSettingsIpcData`) must be extended to include both.

### Health System (`crosshook-core/src/profile/health.rs`)

`ProfileHealthReport` gains a `dependency_status: DependencyHealthStatus` field:
- `DependencyHealthStatus::Ok` — all declared packages are `installed`
- `DependencyHealthStatus::Missing` — one or more packages are `missing`
- `DependencyHealthStatus::Unknown` — one or more packages are `unknown` (winetricks.log absent)
- `DependencyHealthStatus::Unchecked` — never been checked or TTL expired
- `DependencyHealthStatus::NoDeclared` — profile declares no dependencies (no indicator shown)

`deps_missing` and `deps_unknown` map to `HealthStatus::Stale` (amber), not `HealthStatus::Broken` (red), per BR-6.

### SQLite Migrations (`crosshook-core/src/metadata/migrations.rs`)

A new migration (schema version 15) creates `prefix_dependency_states`:

```sql
CREATE TABLE IF NOT EXISTS prefix_dependency_states (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id      TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    package_name    TEXT NOT NULL,
    source          TEXT NOT NULL DEFAULT 'declared',
    status          TEXT NOT NULL DEFAULT 'unchecked',
    checked_at      TEXT,
    installed_at    TEXT,
    install_error   TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    UNIQUE(profile_id, package_name)
);
CREATE INDEX IF NOT EXISTS idx_prefix_dependency_states_profile_id
    ON prefix_dependency_states(profile_id);
```

### Launch Orchestration (`src-tauri/src/commands/launch.rs`)

The launch command checks for unresolved missing dependencies before spawning the game/trainer process. The check queries SQLite for packages with `status = 'missing'` and fresh `checked_at` (< 24h). If blocking deps exist:
- `auto_install_prefix_deps` = true: acquire per-prefix lock, trigger install, wait, then spawn.
- Otherwise: return a structured `LaunchValidationIssue` of type `MissingPrefixDependencies` (severity: `Warning`, not `Fatal`) that the frontend handles by showing the install prompt. The user can dismiss and launch anyway.

`unknown` status packages do NOT block launch — they only surface as amber indicators.

### Community Import Preview (`crosshook-core/src/profile/exchange.rs`)

`CommunityImportPreview` gains a `required_prefix_deps: Vec<String>` field populated from the manifest's `required_protontricks`. The frontend import wizard renders this as a "Required prefix dependencies" section with the BR-16 trust disclosure including the network download warning. Import is not completed until the user acknowledges the disclosure.

### Tauri IPC Commands (new module `src-tauri/src/commands/protontricks.rs`)

New commands:
- `check_prefix_dependencies(profile_name: String) -> Result<Vec<DependencyCheckResult>, String>`
- `install_prefix_dependencies(profile_name: String, packages: Vec<String>) -> Result<(), String>`
- `get_prefix_dependency_states(profile_name: String) -> Result<Vec<PrefixDependencyStateRow>, String>`
- `get_protontricks_binary_status() -> Result<ProtontricksBinaryStatus, String>` (returns found path or not-found reason)

Progress events emitted during install:
- `protontricks://install_started { profile_name, packages }`
- `protontricks://install_progress { profile_name, line }` (stdout/stderr lines)
- `protontricks://install_completed { profile_name, packages }`
- `protontricks://install_failed { profile_name, packages, error }`

---

## Persistence Classification

| Datum | Classification | Rationale |
|---|---|---|
| `required_protontricks` (community) | TOML profile (`[dependencies]`) | Community-shareable; version-controlled in community taps |
| `user_extra_protontricks` | TOML profile (`[dependencies]`) | User-editable per-profile local config |
| `protontricks_binary_path` | TOML settings (`settings.toml`) | User-editable in Settings UI |
| `auto_install_prefix_deps` | TOML settings (`settings.toml`) | User-editable in Settings UI |
| `PrefixDependencyState` rows | SQLite `prefix_dependency_states` | Operational metadata; survives restarts; TTL-gated (24h) |
| Active install progress | Runtime-only (memory + Tauri events) | Ephemeral; not meaningful to persist across restarts |
| Per-prefix install lock | Runtime-only (in-memory mutex) | Session-scoped concurrency gate; meaningless across restarts |

### Persistence / Usability

**Migration and backward compatibility:** `DependenciesSection` is `#[serde(default)]` in `GameProfile`. Existing profiles without a `[dependencies]` section will deserialize cleanly with empty lists. Community schema v1 profiles missing the field are accepted without error. SQLite migration 15 adds the new table; existing databases upgrade automatically on first launch. Old CrossHook versions (pre-v2 community schema) will silently ignore `required_protontricks` — this is acceptable and documented in the changelog.

**Offline behavior:** Dependency check operates against `$WINEPREFIX/winetricks.log` — no network required. Dependency install (winetricks) requires internet access to download packages. CrossHook does not gate install on its own offline mode flag; winetricks will fail with a network error if the internet is unavailable, and CrossHook surfaces that error output.

**Degraded / failure fallback:** If the winetricks binary is absent, dependency management UI elements are shown but all interactive buttons are disabled, with a "binary not configured" notice linking to Settings. Profile launch still proceeds because missing dependencies are a soft-block (BR-6), not a hard failure.

**User visibility / editability:** `required_protontricks` in community profiles is read-only from the user's perspective (it comes from the profile author). `user_extra_protontricks` is fully user-editable via the Prefix Dependencies panel. Settings fields are editable in the Settings screen. SQLite state is read-only from the UI (displayed as status indicators); the [Check Now] button is the user's mechanism to refresh it.

---

## Success Criteria

1. A community profile with `required_protontricks = ["vcrun2019"]` triggers a dependency check (reads `winetricks.log`) when opened.
2. Missing packages surface a user-visible banner before launch; user can dismiss and launch anyway (soft-block).
3. `[Install]` triggers `WINEPREFIX=<path> winetricks vcrun2019` and shows streaming output. Protontricks path used only when Steam App ID present and Steam running.
4. After successful install, `winetricks.log` is re-read, the dependency is recorded as `installed` in SQLite, and the banner disappears.
5. The health check system reports `deps_missing` (amber, not red) for profiles with unresolved dependencies.
6. Settings allows configuring a custom binary path, with a help note for the Flatpak invocation and a tooltip noting winetricks is preferred.
7. Community profile import shows declared dependencies with size/time estimates and the BR-16 trust/network disclosure.
8. Existing profiles without `[dependencies]` behave identically to today.
9. All package names go through the static allowlist; no arbitrary strings reach the shell.
10. A second concurrent install for the same prefix is rejected with a clear error and the launch button is disabled.
11. Dependency check results are cached for 24 hours; CrossHook does not re-check on every launch.
12. `unknown` status packages (winetricks.log absent) show an amber indicator but do not block launch.
13. If Steam is not running and protontricks is configured, CrossHook falls back to winetricks silently.
14. Install retry is always safe (winetricks is idempotent; already-installed verbs are skipped).

---

## Resolved Decisions (from team review)

**RD-1: Launch gate severity — soft-block, not hard-block.**
Missing dependencies map to `HealthStatus::Stale` (amber warning), not `HealthStatus::Broken` (red error). The user can dismiss the "Missing dependencies" prompt and launch anyway. Rationale: CrossHook's detection heuristics are imperfect; hard-blocking would cause false refusals when the user has already installed the dependency through another mechanism. See BR-6.

**RD-2: TTL-based check cache — 24-hour freshness window.**
Dependency check results are trusted for 24 hours without re-verification. This balances not hammering the prefix filesystem on every launch against surfacing stale state quickly enough for day-to-day use. The user can force a re-check at any time via [Check Now]. See BR-5.

**RD-3: Flatpak protontricks — supported but requires explicit configuration.**
Flatpak variant is a first-class support target but cannot be auto-detected. Users must paste `flatpak run com.github.Matoking.protontricks` into Settings. The Settings screen displays this string as a help note, with specific call-out for Steam Deck / Flatpak Steam users. See BR-15.

**RD-4: Concurrent install prevention — per-prefix mutex + UI-level launch disable.**
The per-prefix install lock is an in-memory mutex keyed by absolute prefix path. While a lock is held, the launch button for all profiles sharing that prefix is disabled in the UI. No global install queue is needed; different prefixes may install concurrently. See BR-8, BR-9.

**RD-5: Community schema version bump — forward-compatible, old clients silently skip.**
`COMMUNITY_PROFILE_SCHEMA_VERSION` bumps to 2. Old clients drop unknown fields on deserialization (TOML forward-compatibility) and simply won't manage dependencies. This is documented in the release changelog, not treated as a breaking change. See BR-13.

**RD-6: Static allowlist — embedded in CrossHook, not derived at runtime.**
The package name allowlist is compiled into the binary. It covers the verbs trainers commonly need (confirmed against winetricks). Profile authors wanting an unlisted package submit a request via the issue tracker. This keeps the security boundary explicit and avoids requiring the winetricks binary at startup. See BR-1.

**RD-7: winetricks is the preferred binary; protontricks is secondary.**
winetricks alone (with `WINEPREFIX` env var) is the standard invocation because it does not require Steam to be running. protontricks is used only when the user has it configured, `steam.app_id` is present, and Steam is running — with automatic fallback to winetricks otherwise. This reduces the installation burden for most users. See BR-2, BR-4, BR-18.

**RD-8: Dependency check uses `winetricks.log`, not registry probing.**
`$WINEPREFIX/winetricks.log` is the primary source of truth for installed verbs. It is fast, reliable, and the canonical mechanism winetricks itself uses to track state. Registry probing is not needed for the allowlisted verb set. See BR-5, EC-3.

---

## Open Questions

1. **Shared prefix warning UX**: When multiple profiles share a prefix, how prominently should the warning be surfaced? Inline in the dependency panel is clear; a modal on every install may be too noisy for power users.

2. **Trust disclosure frequency**: The BR-16 trust disclosure fires once per import. Should it also fire once per new `required_protontricks` entry detected after a community tap sync (i.e. when a profile author adds a new package to an existing profile the user already imported)?

3. **Window close during long install (EC-9)**: When the CrossHook window closes while winetricks is running a long install (e.g. dotnet48 at 15 min), the child process continues. On next app open, the install lock is gone (runtime-only) and the state in SQLite is stale. Should CrossHook attempt to detect a running winetricks process on startup and re-attach to it, or simply mark affected packages `unchecked` and let the next health pass re-check via `winetricks.log`? The re-check approach is simpler; re-attach adds complexity but improves UX for very long installs.
