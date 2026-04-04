# Feature Spec: Protontricks / Winetricks Integration

## Executive Summary

CrossHook will integrate winetricks and protontricks to manage WINE prefix dependencies (Visual C++ redistributables, .NET Framework, DirectX, etc.) required by game trainers. Community profiles can declare `required_protontricks` packages; CrossHook detects missing dependencies, presents a guided install flow with streaming console output, and tracks installation state in SQLite. The primary tool is **winetricks-direct** (WINEPREFIX-based invocation) since CrossHook already stores prefix paths — protontricks' Steam App ID resolution is redundant. Protontricks is supported as a user-configured alternative. The implementation adds a `prefix_deps` module to `crosshook-core` (4 files, no new crates), a SQLite migration (v14 to v15), TOML schema extensions, and 4 new Tauri IPC commands. The dominant security concern — command injection via community profile package names — is mitigated by structural validation, `Command::arg()` (no shell), and a `--` flag separator.

## External Dependencies

### CLIs and Tools

#### Winetricks (Primary)

- **Documentation**: [GitHub](https://github.com/Winetricks/winetricks), [Man page](https://www.mankier.com/1/winetricks)
- **Invocation**: `WINEPREFIX=/path/to/pfx winetricks -q <verb1> <verb2> ...`
- **Detection**: `WINEPREFIX=/path/to/pfx winetricks list-installed` — outputs newline/space-delimited installed verb names
- **Key Flags**: `-q` (unattended), `-f` (force reinstall — **never use programmatically**), `-v` (verbose)
- **Exit Codes**: `0` = success, non-zero = failure (POSIX convention)
- **Network**: Downloads from `download.microsoft.com`, `download.visualstudio.microsoft.com`, Archive.org mirrors. Cached at `~/.cache/winetricks/`

#### Protontricks (Secondary, User-Configured)

- **Documentation**: [GitHub](https://github.com/Matoking/protontricks)
- **Invocation**: `protontricks [--no-bwrap] <APPID> -q <verb1> <verb2> ...`
- **Key Flags**: `--no-bwrap` (disable bubblewrap sandbox), `-l` (list games), `-s` (search)
- **Requires**: Steam running, valid Steam App ID
- **Flatpak**: `flatpak run com.github.Matoking.protontricks` — requires manual configuration in Settings

#### Common Verbs for Trainers

| Verb        | Description                          | Typical Duration |
| ----------- | ------------------------------------ | ---------------- |
| `vcrun2019` | Visual C++ 2015-2019 redistributable | 2-5 min          |
| `vcrun2022` | Visual C++ 2022 redistributable      | 2-5 min          |
| `dotnet48`  | .NET Framework 4.8                   | 10-20 min        |
| `d3dx9`     | DirectX 9 redistributable DLLs       | 1-3 min          |
| `corefonts` | Core Microsoft fonts                 | 1-2 min          |
| `xact`      | Microsoft XACT audio runtime         | 1-2 min          |

Full verb list: [winetricks/files/verbs/all.txt](https://github.com/Winetricks/winetricks/blob/master/files/verbs/all.txt)

### Libraries and SDKs

| Library                   | Version   | Purpose                    | Status                |
| ------------------------- | --------- | -------------------------- | --------------------- |
| `tokio::process::Command` | (in-tree) | Async subprocess execution | Already imported      |
| `rusqlite` (bundled)      | (in-tree) | SQLite state persistence   | Already in Cargo.toml |
| `serde` / `toml`          | (in-tree) | TOML profile serialization | Already in Cargo.toml |

**No new crates required.** Binary detection uses the in-tree PATH walk pattern from `resolve_umu_run_path()` in `launch/runtime_helpers.rs:302`.

## Business Requirements

### User Stories

**Primary User: Gamer Running Trainers via CrossHook**

- **US-1**: As a gamer opening a profile for the first time, I want CrossHook to tell me which prefix dependencies are missing and offer to install them, so I don't have to figure out protontricks commands myself.
- **US-2**: As a gamer importing a community profile, I want declared `required_protontricks` packages to be checked and installable before first launch, so the trainer works out of the box.
- **US-3**: As a power user, I want a Prefix Dependencies panel to manually trigger installs, view status, and add packages beyond what the profile declares.
- **US-4**: As a gamer with many profiles, I want the health check to flag profiles missing declared dependencies with an amber indicator.

**Secondary User: Community Profile Author**

- **US-5**: As a profile author, I want to add `required_protontricks = ["vcrun2019", "dotnet48"]` to my profile TOML so other users don't have to reverse-engineer what my trainer needs.

**Power User**

- **US-6**: As a user with non-standard winetricks/protontricks installations, I want to configure binary paths in Settings.

### Business Rules

**BR-1: Package Name Validation** — Package names must pass structural validation (`^[a-z0-9][a-z0-9_\-]{0,63}$`), reject `-`-prefixed strings (flag injection), and enforce max 50 verbs per profile. A curated known-verb set provides WARNING-level advisories for unknown-but-structurally-valid names. Validation runs at both tap sync time and install time.

**BR-2: Binary Discovery** — Discovery order: (1) user-configured path in `settings.toml`, (2) `winetricks` on `$PATH`, (3) `protontricks` on `$PATH`. If absent, install operations are blocked with guidance; profiles still load normally.

**BR-3: Prefix Path Required** — Dependency operations require a non-empty prefix path from the profile. Profiles without a prefix show a "prefix not configured" notice with install buttons disabled.

**BR-4: Winetricks-Direct as Primary** — CrossHook uses `WINEPREFIX=<path> winetricks -q <verbs>` as the default. When the user configures protontricks and the profile has a Steam App ID, `protontricks <appid> -q <verbs>` is used instead. The tool is selectable per-settings, not per-profile.

**BR-5: Dependency Check is TTL-Gated** — Check results cached in SQLite are fresh for 24 hours. After TTL expiry, status reverts to `unknown` for the next health pass. Users can force re-check via [Check Now].

**BR-6: Missing Dependency is a Soft-Block** — Missing dependencies produce an amber warning (`HealthStatus::Stale`), not a launch-blocking error. Users can dismiss the prompt and launch anyway. Rationale: CrossHook's detection heuristics are imperfect; hard-blocking causes false refusals.

**BR-7: Install Requires User Confirmation** — Every install shows a confirmation dialog listing packages with human-readable labels and slow-install warnings. The `auto_install_prefix_deps` setting (default: off) represents standing consent and bypasses per-install confirmation.

**BR-8: One Active Install at a Time** — A global async mutex prevents concurrent installs. While an install is running, all install buttons across all profiles are disabled.

**BR-9: Installation is Atomic Per Batch** — All packages in a single install are passed to winetricks in one invocation. On failure, all packages are marked `install_failed`. Error output is captured for display.

**BR-10: Dependency State Persists** — Check results and install outcomes survive app restarts in SQLite. Active install progress is runtime-only (streamed via Tauri events).

**BR-11: No Community Schema Version Bump** — Adding `required_protontricks` with `#[serde(default)]` does not require bumping `COMMUNITY_PROFILE_SCHEMA_VERSION`. Old clients silently ignore the new field. Old profiles without the field deserialize as empty vec.

**BR-12: Community Trust Disclosure** — On importing a profile with non-empty `required_protontricks`, the import preview must display the package list with a notice: "This profile will install packages into your WINE prefix. Only import profiles from sources you trust." User must acknowledge before completing import.

**BR-13: User-Added Packages** — Users can add extra packages via the manual panel. These are stored in `LocalOverrideTrainerSection.extra_protontricks` (stripped on community export). The effective package list = `required_protontricks` + `extra_protontricks` (deduplicated).

**BR-14: Flatpak Support** — Flatpak winetricks/protontricks is supported but requires explicit path configuration. Settings displays help text with the exact invocation string.

### Edge Cases

| Scenario                                        | Expected Behavior                                                  |
| ----------------------------------------------- | ------------------------------------------------------------------ |
| Prefix not initialized (no `pfx/` subdirectory) | Block installs, show "Launch once to initialize prefix"            |
| Duplicate package names in profile              | Silently deduplicated                                              |
| Package already installed externally            | `list-installed` check detects it; CrossHook marks as `installed`  |
| Protontricks requires interactive Wine dialogs  | Run with host `DISPLAY`; show "Wine dialogs may appear" note       |
| DISPLAY/WAYLAND_DISPLAY unset                   | Block installs with "No display environment available"             |
| Two profiles share same prefix                  | Dependency states are per-profile; shared-prefix warning shown     |
| Offline mode                                    | Detection/install work locally; winetricks verb downloads may fail |
| `list-installed` output format changes          | Fall back to exit-code-only detection                              |

### Success Criteria

- [ ] Community profile with `required_protontricks = ["vcrun2019"]` triggers dependency check on profile open
- [ ] Missing packages surface an amber banner with [Install] / [Skip] before launch
- [ ] Install triggers winetricks (or protontricks) with streaming console output
- [ ] After successful install, package recorded as `installed` in SQLite; banner disappears
- [ ] Health check shows amber indicator for profiles with unresolved dependencies
- [ ] Settings allows configuring winetricks/protontricks binary paths with live validation
- [ ] Community import preview shows declared dependencies with trust disclosure
- [ ] Existing profiles without `[dependencies]` behave identically to today
- [ ] All package names pass structural validation before reaching any subprocess
- [ ] Concurrent installs rejected with clear error; install buttons disabled globally
- [ ] `unknown` status packages show amber indicator but do not block launch
- [ ] Cancellation not exposed in v1 (unsafe mid-install); slow-install warning shown pre-confirmation

## Technical Specifications

### Architecture Overview

```
UI (React)
  │  invoke("detect_protontricks_binary")
  │  invoke("check_prefix_dependencies", { profile_name, prefix_path, ... })
  │  invoke("install_prefix_dependency", { profile_name, prefix_path, package, ... })
  │  invoke("get_dependency_status", { profile_name })
  ▼
src-tauri/src/commands/prefix_deps.rs       ← thin IPC layer
  │
  ▼
crosshook-core/src/prefix_deps/             ← all business logic
  ├── mod.rs                                ← public re-exports, path resolution
  ├── runner.rs                             ← ProtontricksRunner trait, Command building
  ├── store.rs                              ← SQLite state helpers (or inline in metadata/)
  └── validation.rs                         ← validate_protontricks_verbs()
  │
  ▼
MetadataStore (SQLite)                      ← prefix_dependency_state table (v15)
AppSettingsData (TOML)                      ← protontricks_binary_path, winetricks_path
GameProfile::TrainerSection (TOML)          ← required_protontricks field
```

### Data Models

#### SQLite: `prefix_dependency_state` (Migration v14 → v15)

```sql
CREATE TABLE IF NOT EXISTS prefix_dependency_state (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id       TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    package_name     TEXT NOT NULL,
    prefix_path      TEXT NOT NULL,
    state            TEXT NOT NULL DEFAULT 'unknown',
    checked_at       TEXT,
    installed_at     TEXT,
    last_error       TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_package_prefix
    ON prefix_dependency_state(profile_id, package_name, prefix_path);
CREATE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_id
    ON prefix_dependency_state(profile_id);
```

**States**: `unknown` | `installed` | `missing` | `install_failed` | `user_skipped` | `check_failed`

**Key**: `(profile_id, package_name, prefix_path)` — `prefix_path` ensures state invalidates when the user reconfigures the prefix.

#### TOML: `TrainerSection` Extension

```rust
#[serde(rename = "required_protontricks", default, skip_serializing_if = "Vec::is_empty")]
pub required_protontricks: Vec<String>,  // community-declared; exported
```

#### TOML: `LocalOverrideTrainerSection` Extension

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub extra_protontricks: Vec<String>,  // user-added; stripped on community export
```

#### TOML: `AppSettingsData` Extension

```rust
#[serde(default, skip_serializing_if = "String::is_empty")]
pub protontricks_binary_path: String,    // empty = auto-detect

#[serde(default)]
pub auto_install_prefix_deps: bool,      // default false
```

#### Rust Types

```rust
// crosshook-core/src/prefix_deps/models.rs

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DependencyState {
    #[default]
    Unknown,
    Installed,
    Missing,
    InstallFailed,
    CheckFailed,
    UserSkipped,
}

pub struct BinaryDetectionResult {
    pub found: bool,
    pub binary_path: Option<String>,
    pub binary_name: String,
    pub tool_type: Option<PrefixDepsTool>,  // Winetricks or Protontricks
    pub source: String,                     // "settings" | "path" | "not_found"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixDepsTool {
    Winetricks,
    Protontricks,
}

pub enum PrefixDepsError {
    BinaryNotFound { tool: String },
    PrefixNotInitialized { path: String },
    ValidationError(String),
    ProcessFailed { exit_code: Option<i32>, stderr: String },
    Timeout { seconds: u64 },
    AlreadyInstalling { prefix_path: String },
    Database { action: &'static str, source: rusqlite::Error },
}
```

### API Design

#### `detect_protontricks_binary` (sync)

Returns `BinaryDetectionResult { found, binary_path, binary_name, tool_type, source }`. Detection: settings → PATH winetricks → PATH protontricks → Flatpak protontricks → not found.

#### `check_prefix_dependencies` (async)

Validates inputs, resolves binary, runs `winetricks list-installed` against prefix, diffs against declared packages, upserts state to SQLite, returns `CheckPrefixDepsResult { states, all_installed, missing_packages }`. 30-second timeout per check. **Note**: also injects `ProfileStore` state for profile name to profile ID resolution.

#### `install_prefix_dependency` (async)

Validates, acquires global install lock, builds `Command` (apply_host_environment → WINEPREFIX/STEAM_COMPAT_DATA_PATH → `--` separator → verbs), spawns with `attach_log_stdio`, streams `prefix-dep-log` events, emits `prefix-dep-complete` on exit, upserts SQLite state. 300-second timeout. **Note**: also injects `ProfileStore` state for profile name to profile ID resolution.

#### `get_dependency_status` (sync)

Pure SQLite read — returns cached `Vec<PrefixDependencyStatus>` for a profile filtered by prefix path. Params: `profile_name: String`, `prefix_path: String`. No process spawning.

### Command Construction Pattern

Follows the established three-step pattern from `install/service.rs:108-112`:

```rust
let mut cmd = Command::new(binary_path);
// NOTE: Do NOT use env_clear() — winetricks/protontricks need HOME, USER, PATH, XDG_RUNTIME_DIR
apply_host_environment(&mut cmd);             // Restore POSIX env vars needed by winetricks
cmd.env("WINEPREFIX", resolved_prefix);       // Step 2: set prefix-specific vars
cmd.env("STEAM_COMPAT_DATA_PATH", compat_data_path);
cmd.arg("-q");                                // unattended
cmd.arg("--");                                // flag injection prevention (S-06)
for verb in &validated_verbs {
    cmd.arg(verb);
}
cmd.kill_on_drop(true);
attach_log_stdio(&mut cmd, &log_path)?;
```

### System Integration

#### Files to Create

| File                                           | Purpose                                                                  |
| ---------------------------------------------- | ------------------------------------------------------------------------ |
| `crosshook-core/src/prefix_deps/mod.rs`        | Module definition, path resolution, public re-exports                    |
| `crosshook-core/src/prefix_deps/runner.rs`     | `ProtontricksRunner` trait, `RealRunner`/`FakeRunner`, Command building  |
| `crosshook-core/src/prefix_deps/store.rs`      | SQLite state helpers (or `metadata/prefix_deps_store.rs`)                |
| `crosshook-core/src/prefix_deps/validation.rs` | `validate_protontricks_verbs()` — structural regex + known-verb advisory |
| `src-tauri/src/commands/prefix_deps.rs`        | 4 IPC commands + `PrefixDepsInstallState`                                |

#### Files to Modify

| File                                        | Change                                                                                                 |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| `crosshook-core/src/lib.rs`                 | Add `pub mod prefix_deps;`                                                                             |
| `crosshook-core/src/profile/models.rs`      | Add `required_protontricks` to `TrainerSection`, `extra_protontricks` to `LocalOverrideTrainerSection` |
| `crosshook-core/src/settings/mod.rs`        | Add `protontricks_binary_path`, `auto_install_prefix_deps` to `AppSettingsData`                        |
| `crosshook-core/src/metadata/migrations.rs` | Add `migrate_14_to_15()`                                                                               |
| `crosshook-core/src/metadata/mod.rs`        | Add store methods for prefix dependency state                                                          |
| `crosshook-core/src/profile/exchange.rs`    | Add `required_prefix_deps` to `CommunityImportPreview`                                                 |
| `src-tauri/src/commands/mod.rs`             | Add `pub mod prefix_deps;`                                                                             |
| `src-tauri/src/lib.rs`                      | Register 4 commands, manage `PrefixDepsInstallState`                                                   |

## UX Considerations

### User Workflows

#### Primary: First-Time Dependency Setup

1. User opens profile with `required_protontricks` declared
2. CrossHook loads cached state from SQLite (instant); shows status chips per package
3. If stale or never checked, background `check_prefix_dependencies` runs (3-30s)
4. Missing packages surface a "Missing dependencies" banner with **[Install]** / **[Skip]**
5. **[Install]** shows confirmation dialog (package list, slow-install warnings, human-readable labels)
6. On confirm: streams output to ConsoleDrawer, updates chips in real-time
7. On success: banner disappears, chips green. On failure: chips show error with **[Retry]**

#### Pre-Launch Gate

When launching with missing deps and `auto_install_prefix_deps` off (default):

- Modal: "This profile requires prefix dependencies that are not installed"
- **[Install + Launch]** / **[Skip and Launch]** / **[Cancel]**
- Skip records `user_skipped` in SQLite; amber indicator persists

#### Community Profile Import

Import preview includes "Required prefix dependencies" section with trust disclosure notice. Import proceeds regardless — dependency management happens after import.

### UI Patterns

| Component                 | Pattern                                         | Notes                                                                                 |
| ------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------------------- |
| `DependencyStatusBadge`   | Status chip with icon + label                   | Own `DepStatus` type; reuses `crosshook-status-chip` CSS. Do NOT extend `HealthBadge` |
| ConsoleDrawer integration | `prefix-dep-log` event channel                  | Auto-expands on first line; ANSI stripped in Rust                                     |
| Settings binary path      | Text input + Browse + live validation indicator | Flatpak help note with exact invocation string                                        |
| Install confirmation      | Modal with verb list + slow-install warnings    | Required before every install (bypassed by `auto_install_prefix_deps`)                |
| Progress indication       | Indeterminate progress bar above ConsoleDrawer  | No percentage — protontricks lacks structured progress events                         |

### Accessibility

- `<section aria-label="Prefix dependencies">` wrapper
- `role="status"` on install status text (live region)
- `aria-live="polite"` on package list for chip state changes
- Indeterminate `<progress>` element during installs
- `aria-disabled` (not HTML `disabled`) on blocked buttons for screen reader announcement
- Every chip includes both icon and text label — no color-only status indication

### Error Handling

All user-facing messages are **templated strings** — raw subprocess output never reaches the UI (S-11). Stderr goes to CrossHook's internal log only.

| Error                  | User Message                                                      | Recovery             |
| ---------------------- | ----------------------------------------------------------------- | -------------------- |
| Binary not found       | "winetricks is not installed. Install via your package manager."  | Link to Settings     |
| Network timeout        | "Dependency download timed out. Check your connection."           | Retry button         |
| Install failed         | "Dependency installation failed. See logs for details."           | Per-package [Retry]  |
| Install timeout (300s) | "Installation timed out after 5 minutes."                         | Retry button         |
| Prefix not found       | "Wine prefix not found. Set the prefix path in Profile settings." | Navigate to settings |
| Prefix not initialized | "Run a launch first to initialize the prefix."                    | Informational        |
| DISPLAY not set        | "Cannot install — no display environment available."              | Informational        |
| Concurrent install     | "An installation is already in progress."                         | Button disabled      |

## Recommendations

### Implementation Approach

**Recommended Strategy**: Winetricks-direct as primary tool, lazy on-demand model, 5 independently mergeable phases.

**Phasing:**

1. **Phase 1 — Foundation** (Low complexity): Binary detection (`resolve_winetricks_path()`, `resolve_protontricks_path()`), TOML schema additions (`required_protontricks` in `TrainerSection`), onboarding readiness check. No UI.
2. **Phase 2 — Storage** (Low complexity): SQLite migration v14→v15, `prefix_deps_store.rs` with upsert/load/check functions. Isolated, fully testable.
3. **Phase 3 — Install Runner** (Medium complexity): `ProtontricksRunner` trait with `RealRunner`/`FakeRunner`, `validation.rs` with structural regex, `winetricks list-installed` pre-check, Command building with `--` separator, `attach_log_stdio` output streaming, 300-second timeout, per-prefix install lock.
4. **Phase 4 — Health Integration** (Low complexity): Dependency enrichment in `batch_check_health_with_enrich` closure (synchronous SQLite reads only), `ProfileHealthReport` gains dependency status.
5. **Phase 5 — IPC + UI** (Medium complexity): 4 Tauri IPC commands, Settings panel fields, `DependencyStatusBadge` component, ConsoleDrawer integration, pre-launch gate modal, import preview extension, confirmation dialogs.

### Technology Decisions

| Decision             | Recommendation                    | Rationale                                                                           |
| -------------------- | --------------------------------- | ----------------------------------------------------------------------------------- |
| Primary install tool | winetricks-direct                 | CrossHook already stores prefix paths; protontricks' App ID resolution is redundant |
| Process spawning     | `tokio::process::Command`         | Already used throughout launch module                                               |
| Binary detection     | In-tree PATH walk                 | Mirrors `resolve_umu_run_path()` — no `which` crate                                 |
| State persistence    | SQLite migration                  | Follows `health_store.rs` / `offline_store.rs` pattern                              |
| Error types          | Custom enum (not `anyhow`)        | `anyhow` is not in crosshook-core deps; match existing pattern                      |
| Testability          | `ProtontricksRunner` trait        | `FakeRunner` for CI; no real binary required                                        |
| Output streaming     | `attach_log_stdio` + Tauri events | Reuses existing `ConsoleView` / `ConsoleDrawer` infrastructure                      |

### Quick Wins

- Onboarding check for winetricks binary (adds to existing readiness panel)
- `required_protontricks` schema field with `#[serde(default)]` (pure data, no behavior)
- SQLite migration (isolated, testable, zero UI impact)

## Risk Assessment

### Technical Risks

| Risk                                              | Likelihood | Impact | Mitigation                                                   |
| ------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------ |
| `winetricks list-installed` output format changes | Medium     | Medium | Fall back to exit-code-only detection                        |
| Verb detection unreliable for some categories     | High       | Medium | Rely on idempotent install + SQLite cache with TTL           |
| Concurrent installs corrupt WINE prefix           | Medium     | High   | Global async Mutex; UI disables all install buttons          |
| winetricks binary not found                       | Medium     | Medium | Graceful degradation — panel visible but actions disabled    |
| `dotnet48` takes 10-20 minutes                    | High       | Medium | Pre-install slow-install warning; indeterminate progress bar |
| Wine dialogs appear during install                | High       | Low    | "Wine may display installer windows" console note            |
| Flatpak protontricks sandbox issues               | High       | High   | Winetricks-direct primary path avoids this entirely          |
| Uninitialized prefix at install time              | Medium     | Medium | Detect `pfx/` subdirectory; gate with remediation message    |

### Security Considerations

#### Critical — Hard Stops

| Finding                                                          | Risk                                         | Required Mitigation                                                                 |
| ---------------------------------------------------------------- | -------------------------------------------- | ----------------------------------------------------------------------------------- |
| S-01/S-02: Command injection via community profile package names | Arbitrary code execution                     | `Command::new()` with `.arg()` (never shell). Structural validation on all inputs   |
| S-06: Flag injection via `-c` in package names                   | Arbitrary Wine env command execution         | `cmd.arg("--")` separator between App ID and verbs; reject `-`-prefixed strings     |
| S-03: Missing package name validation                            | Uncontrolled input to subprocess             | `validate_protontricks_verbs()` — regex `^[a-z0-9][a-z0-9_\-]{0,63}$`, max 50 verbs |
| S-19: `.args(joined)` regression risk                            | Shell metacharacters in single array element | Always use individual `.arg()` per package; never join                              |
| S-22: Manual UI input requires same validation                   | Bypass of community-TOML validation          | Server-side validation on all IPC entry points; autocomplete constrains UI          |

#### Warnings — Must Address

| Finding                                | Risk                             | Mitigation                                                         |
| -------------------------------------- | -------------------------------- | ------------------------------------------------------------------ |
| S-04: Steam App ID from community TOML | Spoofed App ID                   | App ID must be `u32`, nonzero, from internal game record only      |
| S-07: Environment variable leakage     | Sensitive data in subprocess env | `apply_host_environment()` passes only required POSIX vars; no additional app secrets added |
| S-08: `--force` / checksum bypass      | Skip integrity verification      | Never pass `--force`; checksum failures surface as explicit errors |
| S-10: Concurrent prefix access         | Wine registry corruption         | Per-prefix async Mutex                                             |
| S-11: Raw stderr in UI                 | Path/secret disclosure           | Stderr to tracing log only; templated error messages to UI         |

#### Advisories — Best Practices

- S-12: Audit log for community-triggered installs (deferral: low risk for v1)
- S-13: `--no-bwrap` degrades protontricks isolation (acceptable when winetricks is primary)
- S-14: Prefix path symlink TOCTOU attacks (low practical risk; paths from Steam discovery)

## Task Breakdown Preview

### Phase 1: Foundation

**Focus**: Binary detection, TOML schema, onboarding check
**Tasks**:

- `resolve_winetricks_path()` and `resolve_protontricks_path()` in `prefix_deps/mod.rs`
- `required_protontricks: Vec<String>` in `TrainerSection` with `#[serde(default)]`
- `extra_protontricks: Vec<String>` in `LocalOverrideTrainerSection`
- Winetricks check in `check_system_readiness()` with `Info` severity
- Unit tests for schema round-trip and backward compatibility

**Parallelization**: Binary detection and schema changes are independent.

### Phase 2: Storage

**Focus**: SQLite migration and store module
**Dependencies**: Phase 1 (schema types)
**Tasks**:

- `migrate_14_to_15()` in `metadata/migrations.rs`
- `metadata/prefix_deps_store.rs` with `upsert_*`, `load_*`, `check_*` functions
- Unit tests using `open_in_memory()` from `metadata/db.rs`

### Phase 3: Install Runner

**Focus**: Core install/check logic with security hardening
**Dependencies**: Phase 2 (SQLite store)
**Tasks**:

- `ProtontricksRunner` trait with `RealRunner` / `FakeRunner`
- `validation.rs` with `validate_protontricks_verbs()` — structural regex + known-verb advisory
- Command construction with `apply_host_environment()` → `--` separator (no `env_clear()` — winetricks requires full POSIX env)
- `winetricks list-installed` pre-check for installed package detection
- Prefix initialization guard (check `pfx/` subdirectory)
- 300-second `tokio::time::timeout` on install operations
- `attach_log_stdio()` for output capture
- Global install lock (async Mutex)
- Unit tests using `FakeRunner`

### Phase 4: Health Integration

**Focus**: Integrate with existing health system
**Dependencies**: Phase 3 (runner + store)
**Tasks**:

- Dependency enrichment closure for `batch_check_health_with_enrich`
- `ProfileHealthReport` gains dependency status (synchronous SQLite reads only)
- Shared-prefix detection with `Info`-severity warning
- Health helper: `build_dependency_health_issues(dep_states: &[PrefixDependencyStateRow], required_verbs: &[String], active_prefix: &str) -> Vec<HealthIssue>`

### Phase 5: IPC + UI

**Focus**: Tauri commands, Settings, frontend components
**Dependencies**: Phase 4 (health integration)
**Tasks**:

- `src-tauri/src/commands/prefix_deps.rs` with 4 IPC commands
- Settings: `protontricks_binary_path` field with Browse + live validation + Flatpak help note
- Settings: `auto_install_prefix_deps` toggle (default off)
- `DependencyStatusBadge` component with `DepStatus` type
- ConsoleDrawer: add `prefix-dep-log` event listener
- "Missing dependencies" banner with [Install] / [Skip]
- Pre-launch gate modal with [Install + Launch] / [Skip and Launch] / [Cancel]
- Install confirmation dialog with package list, human-readable labels, slow-install warnings
- Community import preview: `required_prefix_deps` section with trust disclosure
- Per-package [Retry] for `install_failed` state
- Graceful degradation when binary absent (panel visible, actions disabled)

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **Field name: `required_protontricks` vs `required_wine_deps`**
   - Options: Keep `required_protontricks` (familiar community term) vs rename to `required_wine_deps` (tool-agnostic)
   - Impact: New field, no backward compatibility concern. Naming affects community documentation
   - Recommendation: Keep `required_protontricks` — users know the term; winetricks verbs are the canonical naming convention regardless of tool

2. **Static vs dynamic verb allowlist**
   - Options: Hard-coded known-verb list vs runtime `winetricks list` query
   - Impact: Static is simpler and auditable but needs maintenance. Dynamic reduces maintenance but adds startup cost
   - Recommendation: Structural regex as hard gate (blocks injection by construction). Known-verb set as advisory layer only. Static for v1 with refresh plan each release cycle

3. **`user_skipped` reset mechanism**
   - Options: Per-package "Mark as required" action vs panel-level "Reset skip decisions" button
   - Impact: Minor UX difference
   - Recommendation: Per-package action (more precise); additionally, TTL expiry naturally resets `user_skipped` → `unknown`

4. **Dependency state TTL**
   - Options: 24 hours (business-analyzer) vs 7 days (recommendations) vs configurable
   - Impact: Shorter TTL = more re-checks; longer = more stale data
   - Recommendation: 24 hours default, matching the health check staleness model

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Protontricks/winetricks CLI interfaces, Rust process management, integration patterns from Heroic/Lutris/Bottles
- [research-business.md](./research-business.md): 16 business rules, 8 edge cases, 4 workflows, domain model with state machine, persistence classification
- [research-technical.md](./research-technical.md): Architecture design, SQLite schema, 4 IPC commands with request/response contracts, architectural alternatives evaluation
- [research-ux.md](./research-ux.md): User workflows, DependencyStatusBadge design, ConsoleDrawer integration, competitive analysis (Bottles/Lutris/Heroic/Steam), accessibility requirements
- [research-security.md](./research-security.md): 22 findings (5 CRITICAL, 12 WARNING, 5 ADVISORY) — command injection, input validation, subprocess execution, supply chain
- [research-practices.md](./research-practices.md): Reusable code inventory (14 files), module boundaries, KISS assessment, ProtontricksRunner trait for testability, no new crates confirmation
- [research-recommendations.md](./research-recommendations.md): Winetricks-direct rationale, 5-phase strategy, risk assessment, alternative approaches evaluation
