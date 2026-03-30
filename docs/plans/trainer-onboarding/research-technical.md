# Trainer Onboarding — Technical Architecture Specification

## Executive Summary

Trainer onboarding adds first-run guidance, readiness checks, trainer acquisition help, and a guided workflow to CrossHook. The feature builds on existing architectural patterns: the `DiagnosticCollector` pattern for check results, the `useInstallGame` stage-machine pattern for wizard state, TOML settings persistence for onboarding state, and the Tauri IPC command pattern for all backend communication. The design adds a minimal `onboarding/` module in `crosshook-core` (2 files: `mod.rs` + `readiness.rs`), a `commands/onboarding.rs` Tauri command file, reuses the existing `HealthIssue` struct for readiness results, persists completion state in `settings.toml`, and introduces three IPC commands and a frontend modal wizard with supporting hooks and types. Zero new Cargo or npm dependencies are required — all four readiness checks compose existing discovery functions.

---

## Architecture Design

### Component Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│ Frontend (React + TypeScript)                                        │
│                                                                      │
│  ┌─────────────────────┐  ┌──────────────────┐  ┌────────────────┐  │
│  │  OnboardingWizard   │  │ ReadinessCheck-  │  │ TrainerGuidance│  │
│  │  (.tsx)             │  │ list (.tsx)       │  │ (.tsx)         │  │
│  └────────┬────────────┘  └────────┬─────────┘  └───────┬────────┘  │
│           │                        │                     │           │
│  ┌────────┴────────────────────────┴─────────────────────┴────────┐  │
│  │  useOnboarding.ts  (stage-machine state, IPC calls,            │  │
│  │  derived statusText / hintText / actionLabel)                  │  │
│  └────────┬───────────────────────────────────────────────────────┘  │
│           │ invoke()                                                 │
├───────────┼──────────────────────────────────────────────────────────┤
│ Tauri IPC │ (commands/onboarding.rs)                                 │
│           │                                                          │
│  ┌────────┴───────────────────────────────────────────────────────┐  │
│  │  check_readiness  │  dismiss_onboarding  │  get_trainer_guidance│  │
│  └────────┬───────────────────────────────────────────────────────┘  │
│           │                                                          │
├───────────┼──────────────────────────────────────────────────────────┤
│ crosshook-core                                                       │
│           │                                                          │
│  ┌────────┴──────────────────┐                                       │
│  │  onboarding/              │  (minimal: 2 files)                   │
│  │  ├── mod.rs               │  re-exports + types (if few)          │
│  │  └── readiness.rs         │  check functions + inline hints       │
│  └────────┬──────────────────┘                                       │
│           │                                                          │
│  ┌────────┴──────────────────────────────────────────────────────┐   │
│  │  Existing Services (composed, not duplicated):                 │   │
│  │  steam/discovery, steam/auto_populate, steam/proton,          │   │
│  │  profile/health (HealthIssue), settings/mod,                  │   │
│  │  profile/toml_store, install/service                          │   │
│  └───────────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────────┘
```

> **Module placement rationale:** Readiness check logic lives in `crosshook-core/src/onboarding/` (not just in the Tauri command) so it can be shared with the CLI crate (`crosshook-cli`). The module is intentionally minimal — 2 files, no `guidance.rs` or `models.rs` split. Types live in `mod.rs` until >3 types warrant extraction. Guidance hint strings are `&'static str` constants inline in `readiness.rs`. Add `guidance.rs` or `models.rs` only when the 200-line threshold is crossed.

### New Components

#### Backend (`crosshook-core`)

| Component                 | Purpose                                                                                                      |
| ------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `onboarding/mod.rs`       | Module root, re-exports, types (`ReadinessCheckResult`, `TrainerGuidanceContent`)                            |
| `onboarding/readiness.rs` | `check_system_readiness()` free function composing existing discovery + inline `&'static str` hint constants |

#### Backend (`src-tauri`)

| Component                | Purpose                                                                                             |
| ------------------------ | --------------------------------------------------------------------------------------------------- |
| `commands/onboarding.rs` | 3 `#[tauri::command]` IPC handlers wrapping core functions + `get_trainer_guidance` content builder |

#### Frontend

| Component                           | Purpose                                                                          |
| ----------------------------------- | -------------------------------------------------------------------------------- |
| `hooks/useOnboarding.ts`            | Wizard stage-machine, first-run detection, readiness invocation, step completion |
| `types/onboarding.ts`               | TypeScript interfaces for onboarding IPC                                         |
| `components/OnboardingWizard.tsx`   | Modal wizard overlay for first-run                                               |
| `components/ReadinessChecklist.tsx` | Renders per-check status with pass/fail/hint                                     |
| `components/TrainerGuidance.tsx`    | Loading mode explanation and trainer type info                                   |

### Integration Points

1. **App startup** (`src-tauri/src/lib.rs`): After existing startup tasks (auto-load profile, health scan, version scan), emit `onboarding-check` event if `onboarding_completed` is false in settings.
2. **Frontend mount** (`App.tsx`): Listen for `onboarding-check` event and render `OnboardingWizard` modal.
3. **Guided workflow chain**: Wizard orchestrates existing IPC commands in sequence: `auto_populate_steam` -> `profile_save` -> `validate_launch` -> navigate to LaunchPage.
4. **Settings integration**: `onboarding_completed` flag in `AppSettingsData` for fast first-run detection (single boolean, no SQLite dependency).

---

## Data Models

### Persistence: Settings TOML Only (No SQLite)

Onboarding state is persisted entirely via `settings.toml`. No new SQLite migration or table is needed.

**Rationale:** Onboarding is a one-time flow with minimal state. A single boolean covers first-run detection, and the wizard's step-by-step progress is ephemeral frontend state (managed by the `useOnboarding` hook). There is no need for a persistent `onboarding_progress` table — if the user closes mid-wizard, they simply restart it. This avoids adding migration 11 and an `onboarding_store.rs` file for a singleton row that would be written once and never queried again.

### Reuse: `HealthIssue` for Readiness Results

Readiness checks reuse the existing `HealthIssue` struct from `profile/health.rs:31` instead of defining a parallel type. `HealthIssue` already has `field`, `path`, `message`, `remediation`, and `severity` — all the fields needed for readiness check results.

```rust
// Existing in profile/health.rs — reuse as-is
pub struct HealthIssue {
    pub field: String,
    pub path: Option<String>,
    pub message: String,
    pub remediation: String,
    pub severity: HealthSeverity,  // Error, Warning, Info
}
```

### Rust Models (`onboarding/mod.rs`)

Types live in `mod.rs` alongside re-exports. Extract to `models.rs` only if >3 types accumulate.

```rust
use serde::{Deserialize, Serialize};
use crate::profile::health::HealthIssue;

/// Result of running all system-level readiness checks.
/// Individual checks are `HealthIssue` items (reused from profile/health).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessCheckResult {
    pub checks: Vec<HealthIssue>,
    pub all_passed: bool,
    pub critical_failures: usize,
    pub warnings: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerGuidanceEntry {
    pub id: String,
    pub title: String,
    pub description: String,
    pub when_to_use: String,
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerGuidanceContent {
    pub loading_modes: Vec<TrainerGuidanceEntry>,
    pub trainer_sources: Vec<TrainerGuidanceEntry>,
    pub verification_steps: Vec<String>,
}
```

> **Dropped types:** `OnboardingStage`, `OnboardingStatus`, `ReadinessStatus`, `ReadinessCheck` are no longer needed. Wizard stage is frontend-only state (in `useOnboarding.ts`). Readiness items use `HealthIssue` with `HealthSeverity` (Error=fail, Warning=warning, Info=pass/skipped).

### Settings Extension (`settings/mod.rs`)

Add to existing `AppSettingsData`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,  // NEW: fast first-run detection
}
```

### TypeScript Types (`types/onboarding.ts`)

```typescript
import type { HealthIssue } from './health';

export interface ReadinessCheckResult {
  checks: HealthIssue[];
  all_passed: boolean;
  critical_failures: number;
  warnings: number;
}

export interface TrainerGuidanceEntry {
  id: string;
  title: string;
  description: string;
  when_to_use: string;
  examples: string[];
}

export interface TrainerGuidanceContent {
  loading_modes: TrainerGuidanceEntry[];
  trainer_sources: TrainerGuidanceEntry[];
  verification_steps: string[];
}

// Wizard stage is frontend-only state, not persisted
export type OnboardingWizardStage = 'readiness_check' | 'trainer_guidance' | 'profile_creation' | 'completed';
```

---

## API Design

### Tauri IPC Commands (3 commands)

> **Reduced from 5 to 3.** `get_onboarding_status` is unnecessary — the frontend reads `onboarding_completed` from `settings_load` (already exists). `complete_onboarding_step` is unnecessary — wizard step progress is ephemeral frontend state; on completion, the frontend calls `settings_save` with `onboarding_completed: true`.

#### 1. `check_readiness`

Runs all **system-level** first-run readiness checks. No parameters — this checks whether the system is ready for CrossHook, not whether a specific game is configured. Per-game validation (compatdata, proton path) happens later during profile creation via existing `auto_populate_steam` and `validate_launch`.

```rust
#[tauri::command]
pub async fn check_readiness() -> Result<ReadinessCheckResult, String>
```

**Response example:**

```json
{
  "checks": [
    {
      "field": "steam_installed",
      "path": "/home/user/.local/share/Steam",
      "message": "Steam found at ~/.local/share/Steam",
      "remediation": "",
      "severity": "Info"
    },
    {
      "field": "proton_available",
      "path": null,
      "message": "Found 3 Proton versions: GE-Proton9-4, Proton Experimental, Proton 9.0",
      "remediation": "",
      "severity": "Info"
    },
    {
      "field": "game_launched_once",
      "path": null,
      "message": "No compatdata directories detected for any game",
      "remediation": "Launch your game once through Steam before using CrossHook, so Proton creates the WINE prefix.",
      "severity": "Warning"
    },
    {
      "field": "trainer_available",
      "path": null,
      "message": "No trainer path provided yet — download a trainer before creating a profile",
      "remediation": "Download a trainer from FLiNG, WeMod, or another provider before creating a profile.",
      "severity": "Info"
    }
  ],
  "all_passed": false,
  "critical_failures": 0,
  "warnings": 1
}
```

**Readiness checks performed:**

| Check ID (`field`)   | What it verifies                               | Pass criteria (`Info` severity)                                | Existing function reused                                  |
| -------------------- | ---------------------------------------------- | -------------------------------------------------------------- | --------------------------------------------------------- |
| `steam_installed`    | Steam client is installed and accessible       | At least one Steam root candidate found                        | `steam/discovery.rs` → `discover_steam_root_candidates()` |
| `proton_available`   | At least one Proton version is installed       | Non-empty result                                               | `install/service.rs:298` → existing Proton discovery      |
| `game_launched_once` | At least one game has been launched via Proton | Any `steamapps/compatdata/*/pfx` exists                        | Filesystem scan of discovered Steam libraries             |
| `trainer_available`  | Info-only guidance check                       | Always `Info` severity (no trainer path at system check stage) | `install/service.rs:176` → path existence check pattern   |

> **Zero new dependencies confirmed (api-researcher):** All four checks compose existing `crosshook-core` functions. No `reqwest`, `goblin`, `pelite`, or `zip` crates are needed for v1. Those are deferred to future features (Steam Store API enrichment, PE validation, trainer ZIP extraction) per separate issues.

**Why no `game_path` / `app_id` parameters:** `check_readiness` runs before the user has selected a game. It answers "is the system ready for CrossHook at all?" not "is this specific game ready?" Per-game compatdata validation is already handled by `auto_populate_steam` and `validate_launch` during profile creation.

**`game_launched_once` semantic note (business-analyzer):** This check scans for _any_ `compatdata/*/pfx` directory, not a specific game's. A user who has launched _other_ games through Proton will pass this check even if their target game has never been launched. This is intentional — the system-level check is a Proton-infrastructure proxy ("has Proton ever been used?"). The per-game compatdata check happens later in `auto_populate_steam` during profile creation.

#### 2. `dismiss_onboarding`

Permanently dismisses the onboarding wizard by setting `onboarding_completed = true` in settings. User can re-trigger from the Settings page.

```rust
#[tauri::command]
pub fn dismiss_onboarding(
    settings_store: State<'_, SettingsStore>,
) -> Result<(), String>
```

**Logic:**

1. Load current settings.
2. Set `onboarding_completed = true`.
3. Save settings.

#### 3. `get_trainer_guidance`

Returns static compiled guidance content about trainer types and loading modes. Guidance strings are inline in the command handler, not in a separate catalog file.

```rust
#[tauri::command]
pub fn get_trainer_guidance() -> TrainerGuidanceContent
```

**Response example:**

```json
{
  "loading_modes": [
    {
      "id": "source_directory",
      "title": "Source Directory (Default)",
      "description": "Proton reads the trainer directly from its downloaded location on the Linux filesystem. The trainer stays in place and runs via Z:\\ drive mapping.",
      "when_to_use": "Most trainers work with this mode. Use it unless the trainer specifically requires being in the game's directory.",
      "examples": ["FLiNG standalone trainers", "Most WeMod extracted trainers"]
    },
    {
      "id": "copy_to_prefix",
      "title": "Copy to Prefix",
      "description": "CrossHook copies the trainer .exe and its support files (DLLs, configs) into the WINE prefix's C:\\ drive before launch. The trainer sees a Windows-native path.",
      "when_to_use": "Use when the trainer requires DLL side-loading, looks for configs in its own directory, or fails with 'file not found' errors in Source Directory mode.",
      "examples": ["Trainers with companion .dll files", "Trainers that read .ini configs from CWD"]
    }
  ],
  "trainer_sources": [
    {
      "id": "fling",
      "title": "FLiNG Trainers",
      "description": "Standalone .exe trainers. Download from FLiNG's site. Each trainer targets a specific game version.",
      "when_to_use": "Most common choice for CrossHook users. One .exe per game, no installer needed.",
      "examples": ["FLiNG.Trainer.-.Game.Name.v1.2.3.exe"]
    },
    {
      "id": "wemod",
      "title": "WeMod (Extracted)",
      "description": "WeMod trainers must be extracted from the WeMod app data directory. CrossHook cannot use the WeMod launcher directly.",
      "when_to_use": "When you prefer WeMod's trainer database. Requires manual extraction of .exe from WeMod's cache.",
      "examples": ["WeMod app data extraction"]
    }
  ],
  "verification_steps": [
    "Verify the trainer .exe file exists on your Linux filesystem",
    "Ensure the trainer targets the same game version you have installed",
    "For CopyToPrefix mode, check that companion files (.dll, .ini) are in the same directory as the trainer",
    "Launch the game at least once through Steam before using a trainer, so the WINE prefix is initialized"
  ]
}
```

---

## System Constraints

### Performance

| Operation                     | Expected latency | Bottleneck                                           |
| ----------------------------- | ---------------- | ---------------------------------------------------- |
| `check_readiness`             | 50-200ms         | Filesystem scans for Steam roots and Proton installs |
| `dismiss_onboarding`          | <5ms             | Settings TOML write                                  |
| `get_trainer_guidance`        | <1ms             | Static compiled content, no I/O                      |
| First-run detection (startup) | <1ms             | Single boolean check in `AppSettingsData`            |

### Scalability

- Onboarding is a one-time flow with no persistent state beyond a boolean.
- Readiness checks compose existing discovery functions — no new scan patterns.
- Guidance content is O(1) static data.
- No unbounded growth paths.

### Compatibility

- **No SQLite dependency**: Onboarding does not use MetadataStore at all. The `onboarding_completed` flag is in `settings.toml`, which is always available (SettingsStore never fails at init).
- **No Steam installed**: Readiness check reports `Error` severity for `steam_installed` with remediation hint. Wizard continues — user may use `proton_run` or `native` method without Steam.
- **No Proton installed**: Readiness check reports `Error` severity for `proton_available`. Wizard continues but warns that Windows game support requires Proton.
- **Gamepad navigation**: `OnboardingWizard` must integrate with `useGamepadNav` hook for Steam Deck compatibility.
- **macOS**: Steam paths differ (`~/Library/Application Support/Steam`). Existing `discover_steam_root_candidates()` already handles platform differences — readiness checks inherit this.

### UX Constraints (from UX research)

Findings from UX research (`docs/plans/trainer-onboarding/research-ux.md`) with technical assessment:

1. **Readiness check latency vs. streaming**: UX researcher requests per-item streaming via Tauri events. **Technical assessment: not warranted for v1.** Total `check_readiness` latency is 50-200ms — well under the 100ms "instant" threshold for UI feedback. Streaming 4 items over 200ms adds event plumbing and race condition complexity for no perceptible UX benefit. If future checks add network calls (Steam Store API, ProtonDB), revisit with a streaming design. For v1, a single batch response with a brief loading spinner is sufficient.

2. **Re-scan support**: The `check_readiness` command is stateless and re-invokable by design. The `useOnboarding.ts` hook should expose a `recheck()` action that re-invokes the command and updates results in place. No API change needed — this is a frontend-only concern.

3. **Skeleton/timeout handling**: Per WCAG 2.2.2, if any operation exceeds 5 seconds, show a timeout state with retry. Since `check_readiness` is <200ms, this applies only to the wizard's profile creation step (which invokes `auto_populate_steam`, potentially slower). The `useOnboarding.ts` hook should implement a 5-second timeout wrapper around any IPC call that does filesystem scanning.

4. **Trainer path on-blur validation**: Already exists. `validate_optional_trainer_path()` in `install/service.rs:176` checks existence and is-file. The wizard's profile creation step can invoke the existing `validate_install_request` or `validate_launch` Tauri commands for on-blur validation. No new validation command needed.

5. **First-run flag read/set**: Already covered. Reading: `settings_load` (existing command) returns `AppSettingsData.onboarding_completed`. Setting on completion: `settings_save` (existing) with `onboarding_completed: true`. Setting on dismiss: `dismiss_onboarding` (new command). No additional command needed.

6. **Steam scan streaming**: Out of scope for onboarding v1. The existing `auto_populate_steam` command returns a single batch result. Converting to event-based streaming would change the existing API contract. If needed, track as a separate enhancement issue.

---

## Security Requirements

Findings from security research (`docs/plans/trainer-onboarding/research-security.md`).

### Required for v1 (blocking)

1. **Path construction**: All readiness check path construction must use `PathBuf` composition, never string concatenation. This is already the pattern in `steam/discovery.rs`.

2. **Display path sanitization**: All error/remediation messages surfaced to the UI from readiness checks must go through `sanitize_display_path()` (already exists in `commands/shared.rs`). Prevents leaking full home directory paths.

3. **Trainer source URLs**: Trainer source recommendation URLs shown in the onboarding UI (`TrainerGuidanceContent.trainer_sources`) must be compile-time `&'static str` constants, NOT loaded from community tap index data at runtime. This prevents tap authors from injecting malicious URLs into the guidance flow.

4. **IPC capabilities**: Register the 3 new commands in `capabilities/default.json`. `get_trainer_guidance` and `dismiss_onboarding` don't need FS or shell permissions — confirm they are not given broader Tauri permissions than needed.

### Pre-existing issues to fix alongside onboarding (non-blocking but recommended)

5. **W-1 — Git branch argument injection** (`community/taps.rs`): Branch names from community tap subscriptions are passed directly to `git fetch/clone --branch`. A branch name starting with `--` is parsed as a git flag (e.g., `--upload-pack=/evil/script`). Fix: add `--` separator in git arg lists AND validate branch names in `normalize_subscription()` — reject names starting with `-`, allow only `[a-zA-Z0-9/._-]`.

6. **W-2 — Community tap URL scheme allowlist** (`community/taps.rs`): `normalize_subscription()` accepts any non-empty URL including `file://` local paths. Since onboarding will actively encourage users to add community taps, secure defaults matter. Fix: only allow `https://` and `ssh://git@` URL schemes.

### Advisories (future hardening, not blocking v1)

7. **Symlink protection in CopyToPrefix staging** (`launch/script_runner.rs`): `copy_dir_all()` follows symlinks. Add `is_symlink()` skip in the recursive copy loop.

8. **PE header check at trainer file selection**: Add a non-blocking `ValidationSeverity::Warning` for `MZ` magic byte check when user selects trainer file during wizard profile creation step. Only 2-byte `std::fs::File` read, no new dependencies (no `goblin`/`pelite` needed).

9. **Staging path traversal assertion**: `stage_trainer_into_prefix()` is structurally safe (`file_stem()` strips directory components), but add `debug_assert!(staged_directory.starts_with(&staged_root))` for future readers.

---

## Codebase Changes

### Files to Create (8 files)

| File                                                | Layer    | Purpose                                                                                                   |
| --------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/onboarding/mod.rs`       | Core     | Module root, re-exports, types (`ReadinessCheckResult`, `TrainerGuidanceEntry`, `TrainerGuidanceContent`) |
| `crates/crosshook-core/src/onboarding/readiness.rs` | Core     | `check_system_readiness()` free function + inline `&'static str` hint constants                           |
| `src-tauri/src/commands/onboarding.rs`              | Tauri    | 3 IPC commands wrapping core functions + guidance content builder                                         |
| `src/hooks/useOnboarding.ts`                        | Frontend | Wizard stage-machine (follows `useInstallGame.ts` pattern exactly — single hook, no sub-hook split)       |
| `src/types/onboarding.ts`                           | Frontend | TypeScript type definitions                                                                               |
| `src/components/OnboardingWizard.tsx`               | Frontend | Modal wizard overlay                                                                                      |
| `src/components/ReadinessChecklist.tsx`             | Frontend | Readiness check display component                                                                         |
| `src/components/TrainerGuidance.tsx`                | Frontend | Loading mode guidance component                                                                           |

> **Dropped files (vs. original spec):** `onboarding/guidance.rs` (hint strings inline in `readiness.rs`), `onboarding/models.rs` (types in `mod.rs`), `metadata/onboarding_store.rs` (no SQLite), `hooks/useReadinessCheck.ts` (merged into `useOnboarding.ts`), `styles/onboarding.css` (use existing `theme.css` patterns).

### Files to Modify (6 files)

| File                                        | Change                                                                          |
| ------------------------------------------- | ------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs`          | Add `pub mod onboarding;`                                                       |
| `crates/crosshook-core/src/settings/mod.rs` | Add `onboarding_completed: bool` to `AppSettingsData`                           |
| `src-tauri/src/commands/mod.rs`             | Add `pub mod onboarding;`                                                       |
| `src-tauri/src/lib.rs`                      | Register 3 new commands in `invoke_handler`, emit `onboarding-check` at startup |
| `src/App.tsx`                               | Add `onboarding-check` event listener, conditionally render `OnboardingWizard`  |
| `src/types/index.ts`                        | Re-export onboarding types                                                      |

> **No changes to:** `metadata/migrations.rs` (no new table), `metadata/mod.rs` (no onboarding store).

### Dependencies

**Zero new crate or npm dependencies for v1.** All functionality composes existing code:

- `serde` / `serde_json` (serialization, already in use)
- `crate::profile::health::HealthIssue` (reused for readiness checks)
- `crate::steam::discovery` / `crate::steam::proton` (reused for readiness checks)
- `crate::install::service` (path validation patterns)
- `@tauri-apps/api/core` (frontend IPC, already in use)

**Deferred to future issues (not v1):**

- `reqwest` — async HTTP for Steam Store + ProtonDB API enrichment
- `zip` — ZIP extraction for FLiNG trainer archives
- `goblin`/`pelite` — PE file analysis (only if full arch detection or version string extraction is needed beyond the 2-byte `MZ` check)

---

## Technical Decisions

### Decision 1: Onboarding State Persistence

| Option                                  | Pros                                                     | Cons                                                                 |
| --------------------------------------- | -------------------------------------------------------- | -------------------------------------------------------------------- |
| **A: SQLite only**                      | Consistent with metadata pattern, supports complex state | Fails when SQLite unavailable; overkill for a boolean                |
| **B: Settings TOML only (recommended)** | Simple, always available, no migration needed            | Cannot persist step-by-step progress across app restarts             |
| ~~C: Hybrid~~                           | ~~Fast detection + detailed progress~~                   | ~~Two persistence layers for a singleton boolean — over-engineered~~ |

**Recommendation: Option B (Settings TOML only)**. Add `onboarding_completed: bool` to `AppSettingsData`. Wizard step progress is ephemeral frontend state managed by `useOnboarding.ts` — if the user closes mid-wizard, they simply restart it next time.

**Rationale:** The onboarding wizard is a one-time flow. Persisting intermediate wizard steps in SQLite adds a migration, a store file, and coordination logic for a singleton row that would be written once and never queried again. The `settings.toml` approach is simpler, always available (SettingsStore never fails at init), and matches the KISS principle. If analytics on wizard drop-off rates are ever needed, that can be added via the existing `launch_operations` metadata pattern — but this is a future concern, not a launch requirement.

### Decision 2: First-Run Detection

| Option                                      | Latency | Reliability                                            |
| ------------------------------------------- | ------- | ------------------------------------------------------ |
| **A: `settings.onboarding_completed` flag** | ~0ms    | High (TOML always exists)                              |
| B: Empty profile list                       | 5-50ms  | Medium (new install with imported profiles would skip) |
| C: No launch history in SQLite              | 1-5ms   | Medium (SQLite may be unavailable)                     |

**Recommendation: Option A**. Check `settings.onboarding_completed` at startup. Default is `false` for new installations. Existing users upgrading will also get `false` (since the field is new with `#[serde(default)]`), but the readiness check will quickly show all-pass and the wizard can be dismissed in one click.

### Decision 3: Readiness Check Architecture

**Recommendation: Single monolithic `check_readiness()` command**. All readiness checks are fast filesystem operations totaling <200ms. Breaking into individual commands adds IPC overhead without UX benefit. The frontend can show a single loading spinner during the check.

### Decision 4: Wizard Presentation

**Recommendation: Modal overlay for first-run**. A full-screen modal wizard focuses the user's attention and avoids modifying the sidebar navigation. The wizard is dismissible and resumable. After completion or dismissal, contextual help banners can appear on relevant pages (ProfilesPage trainer field, LaunchPage method selector).

### Decision 5: Guided Workflow Implementation

The wizard's final step (profile creation) should orchestrate existing IPC commands rather than introducing new ones:

1. `auto_populate_steam` with user-selected game -> populates Steam fields
2. `profile_save` with wizard-assembled profile -> persists the TOML
3. Navigate to ProfilesPage with new profile selected
4. (Optional) `validate_launch` to pre-validate before first launch

This approach ensures the wizard produces identical results to manual profile creation.

---

## Open Questions

1. **Upgrade path for existing users**: When existing users upgrade and `onboarding_completed` defaults to `false`, should the wizard auto-trigger or only show a dismissible banner? (Recommend: banner only if profiles already exist.)

2. **Re-trigger mechanism**: How does a user re-access the onboarding wizard after dismissal? (Recommend: "Show Onboarding" button in Settings page.)

3. **Trainer download guidance depth**: No trainer site has a public API (FLiNG, WeMod, CheatHappens, MrAntiFun — all zero public APIs, confirmed by api-researcher). Should the guidance include compile-time `&'static str` URLs to these sites, or only general descriptions? Security researcher recommends compile-time constants only (never runtime-loaded from tap index). Liability concern remains with linking to trainer download sites at all.

4. **Gamepad-first UX**: The wizard must work with `useGamepadNav` for Steam Deck. Should it use dedicated gamepad button prompts (A=Next, B=Back, Y=Skip)?

5. **Offline-first**: Does any readiness check require network access? (Confirmed: No. All checks are local filesystem only. Steam Store and ProtonDB APIs are future enrichment, not v1.)

6. **WeMod on Linux context**: WeMod on Linux is supported via [DeckCheatz/wemod-launcher](https://github.com/DeckCheatz/wemod-launcher) (Python, AGPL-3.0), which hooks into Steam `%command%` launch options and manages its own Wine prefix. Should the onboarding wizard detect `wemod-launcher` presence and reference it in guidance? (Recommend: detect and mention in guidance text, but do not integrate programmatically — AGPL-3.0 license is incompatible with direct integration.)

## External API Reference (future features, not v1)

Documented by api-researcher at `docs/plans/trainer-onboarding/research-external.md`. Preserved here for future issue creation:

- **Steam Store API**: `GET https://store.steampowered.com/api/appdetails?appids={appid}` — returns name, header_image, platforms.linux. Rate limit ~200/5min. Could enrich onboarding UI with game artwork.
- **ProtonDB API**: `GET https://www.protondb.com/api/v1/reports/summaries/{appid}.json` — returns tier, confidence, score. No auth. Could show compatibility rating in readiness check.
- **No trainer site APIs exist** — all trainer discovery must be local filesystem scan or user-guided download.

---

## Relevant Files

### Primary (must-read for implementation)

- `crates/crosshook-core/src/profile/health.rs` — `HealthIssue` and `HealthSeverity` types reused for readiness checks
- `crates/crosshook-core/src/steam/auto_populate.rs` — Steam auto-discovery, reuse for readiness checks
- `crates/crosshook-core/src/steam/proton.rs` — Proton discovery, reuse for readiness checks
- `crates/crosshook-core/src/install/service.rs` — Proton discovery at `:298`, path validation at `:176` (both reused)
- `crates/crosshook-core/src/settings/mod.rs` — `AppSettingsData`, `SettingsStore` — add `onboarding_completed` here
- `src-tauri/src/lib.rs` — Startup sequence, state management, command registration
- `src-tauri/src/commands/shared.rs` — `sanitize_display_path()` (security: use for all UI-facing paths)
- `src-tauri/capabilities/default.json` — Register 3 new commands with minimal permissions
- `src/hooks/useInstallGame.ts` — **Stage-machine hook pattern to follow exactly** for `useOnboarding.ts`

### Reference (patterns to follow)

- `crates/crosshook-core/src/steam/diagnostics.rs` — `DiagnosticCollector` pattern for check result accumulation
- `crates/crosshook-core/src/launch/script_runner.rs` — `TrainerLoadingMode`, staging logic, `SUPPORT_DIRECTORIES`
- `crates/crosshook-core/src/profile/models.rs` — `GameProfile`, `TrainerLoadingMode` enum
- `crates/crosshook-core/src/install/models.rs` — `InstallGameRequest`/`Result` pattern
- `src-tauri/src/commands/install.rs` — IPC command pattern (async, spawn_blocking)
- `src-tauri/src/commands/steam.rs` — Steam discovery IPC commands
- `src/types/install.ts` — TypeScript type definition pattern
- `src/types/health.ts` — `HealthIssue` TypeScript interface (reuse for readiness)
- `src/types/settings.ts` — `AppSettingsData` TypeScript interface
- `src/components/pages/ProfilesPage.tsx` — Largest page component, integration target

### Security-relevant (for W-1/W-2 fixes alongside onboarding)

- `crates/crosshook-core/src/community/taps.rs` — `normalize_subscription()`: add branch name validation + URL scheme allowlist
- `crates/crosshook-core/src/launch/script_runner.rs` — `copy_dir_all()`: add symlink skip advisory

## Architectural Patterns

- **DiagnosticCollector**: Used for readiness check result accumulation (`steam/diagnostics.rs`). Readiness checks follow a similar accumulate-then-summarize pattern, but output `Vec<HealthIssue>` instead of defining a parallel type.
- **HealthIssue reuse**: Readiness checks produce `HealthIssue` items (from `profile/health.rs`) with `field` as the check ID (e.g., `"steam_installed"`), `severity` for pass/fail/warn, and `remediation` for user-facing hints. This avoids a parallel `ReadinessCheck` type.
- **Stage-machine hooks**: `useInstallGame.ts` defines `InstallGameStage` union type with derived `statusText`, `hintText`, `actionLabel`. The `useOnboarding.ts` hook must follow this pattern exactly — including debounce, error handling, and derived state calculation.
- **IPC error handling**: All Tauri commands return `Result<T, String>`. Errors are `.map_err(|e| e.to_string())`. No structured errors cross IPC.
- **Startup event cascade**: `lib.rs` setup emits events at staggered delays (350ms profile load, 500ms health scan, 2000ms version scan). Onboarding check should fit at ~100ms (before profile load), reading `onboarding_completed` from settings.
- **Free functions over service structs**: `onboarding/readiness.rs` uses free functions with `Path` arguments, not an `OnboardingService` struct. The Tauri command in `commands/onboarding.rs` is a thin wrapper. This matches the practices-researcher recommendation.
- **Minimal module, grow later**: The `onboarding/` core module starts with 2 files (`mod.rs` + `readiness.rs`). Types live in `mod.rs`. Guidance strings are `&'static str` constants in `readiness.rs`. Add `guidance.rs` or `models.rs` only when the 200-line threshold per file is crossed.
- **Single frontend hook**: `useOnboarding.ts` handles the full wizard flow including readiness checks — no `useReadinessCheck.ts` sub-hook. This matches `useInstallGame.ts` which handles a similarly complex flow (prefix resolution, validation, installer execution, review, save) in one hook.

## Edge Cases

- **Settings file corruption**: If `settings.toml` is corrupted, `AppSettingsData` defaults all fields (including `onboarding_completed = false`), so the wizard re-triggers. This is acceptable — the wizard is lightweight and can be dismissed immediately.
- **Concurrent app instances**: Settings TOML has no concurrency protection, but onboarding is a one-time flow unlikely to race. Worst case: both instances see `onboarding_completed = false` and both show the wizard.
- **Profile import during onboarding**: If user imports profiles via drag-and-drop during wizard, the wizard should not block imports. `profiles-changed` event refreshes any profile list the wizard may show.
- **Upgrade from pre-onboarding version**: `onboarding_completed` defaults to `false` via `#[serde(default)]`. Existing users will see the wizard on first upgrade launch. Since readiness checks will likely all pass, the wizard can be completed or dismissed in one step. See Open Question #1 for upgrade UX.
- **Empty Steam library**: Readiness check for `game_launched_once` scans compatdata directories. A fresh Steam install with no games will report `Warning` severity — the remediation string explains what to do.
- **No Steam at all**: User may be using `proton_run` with a standalone Proton or `native` launch method. `steam_installed` check reports `Error` severity but does not block the wizard. The remediation text explains that Steam is recommended but not strictly required.
