# Architecture Research: trainer-onboarding

## System Overview

CrossHook is a Tauri v2 desktop app with a React 18/TypeScript frontend and a Rust backend split across two crates: `crosshook-core` (all business logic) and `src-tauri` (thin Tauri command shell). All data persists via TOML files (`~/.config/crosshook/`) for profiles and settings, plus a SQLite metadata store for launch history and health snapshots. The onboarding feature adds a new `crosshook-core/src/onboarding/` module, three Tauri commands in `commands/onboarding.rs`, a stage-machine hook `useOnboarding.ts`, and a modal wizard overlay in the React frontend.

## Relevant Components

### Rust — Core Library (`crates/crosshook-core/`)

- `src/lib.rs`: Module registry — add `pub mod onboarding;` here
- `src/settings/mod.rs`: `AppSettingsData` struct (TOML-persisted) — add `onboarding_completed: bool` with `#[serde(default)]`; `SettingsStore` exposes `load()`/`save()` used by Tauri commands
- `src/steam/discovery.rs`: `discover_steam_root_candidates(steam_client_path, diagnostics)` — returns `Vec<PathBuf>` of valid Steam roots (native + Flatpak); used by `steam_installed` readiness check
- `src/steam/proton.rs`: `discover_compat_tools(steam_root_candidates, diagnostics)` — returns `Vec<ProtonInstall>`; used by `proton_available` readiness check
- `src/profile/health.rs`: Defines `HealthIssue { field, path, message, remediation, severity }` and `HealthIssueSeverity { Error, Warning, Info }` — the `ReadinessCheckResult` reuses `Vec<HealthIssue>` directly, no new types needed
- `src/install/service.rs`: `validate_optional_trainer_path()` at line ~32 — patterns for trainer path validation; `is_windows_executable()` at line 292 — `.exe` extension check pattern for MZ validation

### Rust — Tauri Shell (`src-tauri/`)

- `src/lib.rs`: App entry point — initializes stores as managed state, runs startup tasks in async spawns (health scan at 500ms, version scan at 2000ms), emits events (`auto-load-profile`, `profile-health-batch-complete`, `version-scan-complete`); add `onboarding-check` emit here after `settings_store.load()` check
- `src/startup.rs`: `resolve_auto_load_profile_name()` shows the pattern for reading settings at startup; `run_metadata_reconciliation()` shows idiomatic startup side-effect structure
- `src/commands/mod.rs`: Module registry for all command handlers — add `pub mod onboarding;`
- `src/commands/settings.rs`: `settings_load` / `settings_save` commands — shows the minimal command pattern: `State<'_, SettingsStore>` injection, `.load()` / `.save()`, `map_err(|e| e.to_string())`
- `src/commands/steam.rs`: `default_steam_client_install_path()` and `list_proton_installs()` — directly reusable logic for `steam_installed` and `proton_available` readiness checks; `list_proton_installs` already chains `discover_steam_root_candidates` → `discover_compat_tools`
- `capabilities/default.json`: Minimal permissions file (`core:default`, `dialog:default`) — new onboarding commands need no additional FS permissions beyond `core:default`

### React Frontend (`src/`)

- `App.tsx`: Root app shell — `AppShell` holds `route: AppRoute` state; `handleGamepadBack()` queries `[data-crosshook-focus-root="modal"]` for B-button modal close (wizard must use this attribute); the wizard overlay renders at this level, conditionally on `onboarding_completed === false`; add `listen('onboarding-check', ...)` event listener here
- `components/layout/ContentArea.tsx`: Route-to-page mapping via `switch(route)` — exhaustive type check via `never` ensures any new `AppRoute` would cause a compile error; wizard is a modal overlay, not a new route
- `src/types/health.ts`: TypeScript mirror of `HealthIssue` — `{ field, path, message, remediation, severity }` — onboarding types import this directly
- `src/types/index.ts`: Re-exports all type modules — add `export * from './onboarding';`
- `src/hooks/useInstallGame.ts`: **Canonical stage-machine hook pattern** — `stage: InstallGameStage` union type drives UI state; `useCallback` wraps all async actions; `invoke()` calls are inside try/catch with typed error mapping; `reset()` returns to initial state; `useEffect` for side effects triggered by state changes; `useOnboarding.ts` mirrors this pattern exactly

## Data Flow

```
Startup (lib.rs setup block)
  → settings_store.load() checks onboarding_completed
  → emit("onboarding-check", { show: !onboarding_completed })
         ↓
App.tsx listen("onboarding-check")
  → setShowWizard(true) if show === true
         ↓
OnboardingWizard.tsx (modal overlay, data-crosshook-focus-root="modal")
  → useOnboarding.ts (stage: readiness_check → trainer_guidance → profile_creation → completed)
         ↓
Stage: readiness_check
  → invoke("check_readiness")
         ↓
  commands/onboarding.rs::check_readiness()
  → onboarding::readiness::check_system_readiness()
     → steam/discovery.rs::discover_steam_root_candidates()  [steam_installed]
     → steam/proton.rs::discover_compat_tools()              [proton_available]
     → fs::is_dir() scan of steamapps/compatdata/*/pfx       [game_launched_once]
     → always Info                                           [trainer_available]
  → ReadinessCheckResult { checks: Vec<HealthIssue>, all_passed, critical_failures, warnings }
         ↓
Stage: profile_creation (composes existing components)
  → AutoPopulate.tsx → invoke("auto_populate_steam")
  → ProfileFormSections.tsx (fills profile fields)
         ↓
Stage: completed
  → invoke("dismiss_onboarding")
         ↓
  commands/onboarding.rs::dismiss_onboarding(State<SettingsStore>)
  → settings.load() → settings.onboarding_completed = true → settings.save()
```

## Integration Points

### New Files (8)

| File                                                | Layer    | Integration                                                                                                                         |
| --------------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/onboarding/mod.rs`       | Core     | Module root; expose `ReadinessCheckResult`, `TrainerGuidanceContent` types                                                          |
| `crates/crosshook-core/src/onboarding/readiness.rs` | Core     | `check_system_readiness()` calls `discover_steam_root_candidates`, `discover_compat_tools`, filesystem check                        |
| `src-tauri/src/commands/onboarding.rs`              | Tauri    | 3 commands: `check_readiness`, `dismiss_onboarding`, `get_trainer_guidance`; `dismiss_onboarding` takes `State<'_, SettingsStore>`  |
| `src/hooks/useOnboarding.ts`                        | Frontend | Stage-machine following `useInstallGame.ts` pattern; stages: `readiness_check`, `trainer_guidance`, `profile_creation`, `completed` |
| `src/types/onboarding.ts`                           | Frontend | `ReadinessCheckResult`, `OnboardingWizardStage`; imports `HealthIssue` from `./health`                                              |
| `src/components/OnboardingWizard.tsx`               | Frontend | Modal overlay at `App.tsx` level; `role="dialog"`, `aria-modal`, `data-crosshook-focus-root="modal"`                                |
| `src/components/ReadinessChecklist.tsx`             | Frontend | Per-check status cards from `ReadinessCheckResult.checks`                                                                           |
| `src/components/TrainerGuidance.tsx`                | Frontend | Loading mode cards using `CollapsibleSection` component                                                                             |

### Modified Files (6)

| File                                        | Change                                                                                  | Notes                                                                                  |
| ------------------------------------------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs`          | Add `pub mod onboarding;`                                                               | Line 11, after existing modules                                                        |
| `crates/crosshook-core/src/settings/mod.rs` | Add `onboarding_completed: bool` to `AppSettingsData`                                   | `#[serde(default)]` already on struct; existing tests need no update                   |
| `src-tauri/src/commands/mod.rs`             | Add `pub mod onboarding;`                                                               | Line 15, after `pub mod version;`                                                      |
| `src-tauri/src/lib.rs`                      | Register 3 commands in `invoke_handler!`; emit `onboarding-check` in setup block        | Emit pattern mirrors lines 61-70 (`auto-load-profile`); check settings before emitting |
| `src/App.tsx`                               | Listen for `onboarding-check`, manage `showWizard` state, render `<OnboardingWizard />` | `listen()` from `@tauri-apps/api/event`; cleanup in `useEffect` return                 |
| `src/types/index.ts`                        | Add `export * from './onboarding';`                                                     | Line 12                                                                                |

## Key Dependencies

### Internal Rust (crosshook-core reuse)

- `HealthIssue` / `HealthIssueSeverity` from `profile/health.rs` — reused as `ReadinessCheckResult.checks` element type; no new parallel type
- `discover_steam_root_candidates()` from `steam/discovery.rs` — for `steam_installed` check
- `discover_compat_tools()` from `steam/proton.rs` — for `proton_available` check; called indirectly via `list_proton_installs` in `commands/steam.rs`
- `SettingsStore` / `AppSettingsData` from `settings/mod.rs` — `dismiss_onboarding` injects `State<'_, SettingsStore>` via Tauri managed state
- `validate_optional_trainer_path()` from `install/service.rs` — pattern for trainer `.exe` validation in profile creation step

### Frontend (React component reuse)

- `AutoPopulate.tsx` — composable in profile creation step; already triggers `auto_populate_steam` invoke
- `ProfileFormSections.tsx` — composable in profile review step
- `InstallField` component from `ui/InstallField` — file path inputs with label, browse, helpText, error
- `CollapsibleSection` component — for "Which mode?" progressive disclosure cards
- `ControllerPrompts` — extended with `confirmLabel`, `backLabel`, `showBumpers` override props
- `useGamepadNav` — wizard uses `data-crosshook-focus-root="modal"` to trap focus; existing `handleGamepadBack()` in `App.tsx` already handles this

### External Libraries

- `directories` crate (already in `Cargo.toml`) — `BaseDirs::new()` for platform path resolution, used by `SettingsStore`
- `serde` / `serde_json` / `toml` — already present; used for IPC serialization and TOML persistence
- No new Rust dependencies required for v1
- No new npm packages required for v1

## Architectural Gotchas

- **Startup event timing**: `onboarding-check` must be emitted after a short delay (like `auto-load-profile` at 350ms) to ensure the React frontend has mounted and registered its listener before the event fires.
- **Settings re-read**: `dismiss_onboarding` must call `settings_store.load()` before modifying and saving — never assume in-memory state is current (parallel mutations possible).
- **`AppShell` vs `App` level**: The wizard overlay should render inside `ProfileProvider`/`ProfileHealthProvider` (i.e., inside `AppShell`) if it needs to compose `ProfileFormSections`; otherwise it renders at `App` level. Given it uses profile creation, place it inside `AppShell`.
- **`onboarding_completed` default false**: Existing users upgrading will see the wizard; the spec recommends Option B (show banner not modal if profiles already exist) — so the startup emit should check `profile_store.list().len() > 0` and suppress the modal trigger for existing users.
- **No SQLite migration needed**: Persistence is TOML-only; `#[serde(default)]` on `AppSettingsData` handles forward/backward compatibility transparently.
- **Capabilities file**: `capabilities/default.json` uses `core:default` which covers `invoke`. The 3 new commands need only `core:default` — no new capability entries required unless `opener:open-url` is added for "Install Steam" button (W-3).
