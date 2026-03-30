# Trainer Onboarding — Code Analysis

## Executive Summary

The trainer-onboarding feature slots cleanly into three existing patterns: the `SettingsStore` load-mutate-save cycle for persistence, the stage-machine hook pattern (`useInstallGame.ts`) for the wizard, and the portal modal pattern (`ProfileReviewModal.tsx`) for the UI shell. No new infrastructure is required — the backend composes existing Steam/Proton discovery functions and reuses the `HealthIssue` type verbatim.

## Existing Code Structure

### Backend (Rust)

| File                                           | Role for Onboarding                                                        |
| ---------------------------------------------- | -------------------------------------------------------------------------- |
| `crates/crosshook-core/src/settings/mod.rs`    | Add `onboarding_completed: bool` to `AppSettingsData`                      |
| `crates/crosshook-core/src/profile/health.rs`  | `HealthIssue` + `HealthIssueSeverity` — reused as-is for readiness results |
| `crates/crosshook-core/src/steam/discovery.rs` | `discover_steam_root_candidates()` — steam_installed check                 |
| `crates/crosshook-core/src/steam/proton.rs`    | `discover_compat_tools()` — proton_available check                         |
| `crates/crosshook-core/src/lib.rs`             | Module registry — add `pub mod onboarding;`                                |
| `src-tauri/src/commands/settings.rs`           | Canonical sync command pattern to copy                                     |
| `src-tauri/src/commands/steam.rs`              | Discovery command pattern + `spawn_blocking` for async                     |
| `src-tauri/src/commands/mod.rs`                | Command registry — add `pub mod onboarding;`                               |
| `src-tauri/src/lib.rs`                         | Startup event emission + `invoke_handler![]` registration                  |
| `src-tauri/src/startup.rs`                     | Startup settings read pattern                                              |
| `src-tauri/src/commands/shared.rs`             | `sanitize_display_path()` — apply to path fields in IPC responses          |

### Frontend (TypeScript / React)

| File                                    | Role for Onboarding                                               |
| --------------------------------------- | ----------------------------------------------------------------- |
| `src/hooks/useInstallGame.ts`           | **Canonical pattern** — mirror exactly for `useOnboarding.ts`     |
| `src/components/ProfileReviewModal.tsx` | **Portal modal pattern** — mirror for `OnboardingWizard.tsx`      |
| `src/App.tsx`                           | Add `onboarding-check` event listener + conditional wizard render |
| `src/types/health.ts`                   | `HealthIssue` interface — import directly; no new type            |
| `src/types/index.ts`                    | Add `export * from './onboarding';`                               |

---

## Implementation Patterns

### 1. Settings: Add Flag with `#[serde(default)]`

`AppSettingsData` carries a struct-level `#[serde(default)]` attribute. Any new field with a `Default` impl deserializes as the default when the key is absent from TOML. `bool` defaults to `false`.

```rust
// settings/mod.rs — current
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
}

// Add this field — no per-field annotation needed:
    pub onboarding_completed: bool,
```

**Critical**: The struct-level `#[serde(default)]` handles backward compatibility automatically. Existing `settings.toml` files without this key will deserialize with `onboarding_completed = false`.

### 2. Settings: Load-Mutate-Save (never construct fresh default)

```rust
// startup.rs:76 shows the pattern
pub fn resolve_auto_load_profile_name(settings_store, profile_store) {
    let settings = settings_store.load()?;  // load existing
    // read fields...
}

// For marking onboarding complete — the only correct pattern:
let mut settings = store.load().map_err(|e| e.to_string())?;
settings.onboarding_completed = true;
store.save(&settings).map_err(|e| e.to_string())?;

// NEVER:  store.save(&AppSettingsData { onboarding_completed: true, ..Default::default() })
// This would wipe community_taps, auto_load_last_profile, etc.
```

### 3. Tauri Command: Sync Pattern

Exact pattern from `commands/settings.rs` — copy this structure:

```rust
// commands/onboarding.rs
use crosshook_core::settings::{AppSettingsData, SettingsStore, SettingsStoreError};
use tauri::State;

fn map_settings_error(error: SettingsStoreError) -> String {
    error.to_string()
}

#[tauri::command]
pub fn onboarding_check_readiness(
    store: State<'_, SettingsStore>,
) -> Result<OnboardingReadinessResult, String> {
    // ...
}

#[tauri::command]
pub fn onboarding_mark_complete(
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let mut settings = store.load().map_err(map_settings_error)?;
    settings.onboarding_completed = true;
    store.save(&settings).map_err(map_settings_error)
}

// Include a type-assertion test matching commands/settings.rs pattern:
#[cfg(test)]
mod tests {
    #[test]
    fn command_signatures_match_ipc_contract() {
        let _ = onboarding_check_readiness as fn(State<'_, SettingsStore>) -> Result<OnboardingReadinessResult, String>;
        let _ = onboarding_mark_complete as fn(State<'_, SettingsStore>) -> Result<(), String>;
    }
}
```

### 4. Steam Discovery: How to Call for Readiness Checks

`discover_steam_root_candidates` takes an explicit path first (empty = skip), then falls back to home-dir locations:

```rust
// steam.rs:128 shows the empty-string pattern used in startup:
let steam_roots = discover_steam_root_candidates("", &mut diagnostics);
// Non-empty result → steam is installed

// Then for proton:
let proton_installs = discover_compat_tools(&steam_roots, &mut diagnostics);
// Non-empty result → proton is available
```

Both functions populate `&mut Vec<String>` diagnostics. Log them with `tracing::debug!` as `list_proton_installs` does (line 44).

### 5. HealthIssue: Reuse Directly

`HealthIssue` is the exact return type for readiness checks. No parallel type needed.

```rust
// profile/health.rs:31
pub struct HealthIssue {
    pub field: String,    // "steam_installed", "proton_available", etc.
    pub path: String,     // sanitize with sanitize_display_path() before returning
    pub message: String,
    pub remediation: String,
    pub severity: HealthIssueSeverity,  // Error | Warning | Info
}
```

Frontend imports from `src/types/health.ts` — this type already serializes over IPC.

### 6. Startup Event Pattern

From `lib.rs:61-70`, the auto-load-profile event:

```rust
let app_handle = app.handle().clone();
tauri::async_runtime::spawn(async move {
    sleep(Duration::from_millis(350)).await;  // wait for React to mount
    if let Err(error) = app_handle.emit("auto-load-profile", &profile_name) {
        tracing::warn!(%error, "failed to emit auto-load-profile event");
    }
});
```

For `onboarding-check`, use the same 350ms delay. The event fires regardless of onboarding status; the frontend handler checks settings (via `onboarding_check_readiness`) after receiving the event, or the backend includes the result in the event payload.

Existing delays in startup:

- 350ms → `auto-load-profile`
- 500ms → `profile-health-batch-complete`
- 2000ms → `version-scan-complete`

Use **350ms** for `onboarding-check` — it's user-visible and needs to appear early.

### 7. Stage-Machine Hook: Mirror `useInstallGame.ts`

The hook structure to replicate exactly:

```typescript
// Canonical structure from useInstallGame.ts:

// 1. String union stage type (defined in types/install.ts, replicate in types/onboarding.ts)
type OnboardingStage = 'idle' | 'checking' | 'checks_done' | 'wizard_open' | 'complete' | 'failed';

// 2. Pure derive functions at module top (before the hook)
function deriveStatusText(stage: OnboardingStage): string { ... }
function deriveHintText(stage: OnboardingStage): string { ... }

// 3. Factory function for initial state
function createInitialState(): OnboardingState { ... }

// 4. The hook itself
export function useOnboarding(): UseOnboardingResult {
  const [stage, setStageState] = useState<OnboardingStage>('idle');
  // ... other state

  // 5. useCallback-wrapped async driver
  const runChecks = useCallback(async () => {
    setStageState('checking');
    try {
      const result = await invoke<OnboardingReadinessResult>('onboarding_check_readiness');
      setChecksResult(result);
      setStageState('checks_done');
    } catch (err) {
      setStageState('failed');
      setError(normalizeErrorMessage(err));
    }
  }, []);

  // 6. Derived booleans
  return {
    stage,
    isIdle: stage === 'idle',
    isChecking: stage === 'checking',
    // ...
    statusText: deriveStatusText(stage),
    hintText: deriveHintText(stage),
    actionLabel: deriveActionLabel(stage),
    runChecks,
    reset,
  };
}
```

### 8. Portal Modal: Mirror `ProfileReviewModal.tsx`

Key implementation requirements extracted from the component:

```typescript
// 1. Portal host created/destroyed on component mount, NOT on open state
useEffect(() => {
  const host = document.createElement('div');
  host.className = 'crosshook-modal-portal';
  portalHostRef.current = host;
  document.body.appendChild(host);
  setIsMounted(true);
  return () => { host.remove(); portalHostRef.current = null; setIsMounted(false); };
}, []);  // empty deps — once on mount

// 2. Sibling inert + aria-hidden when open (focus trap enforcement)
Array.from(body.children)
  .filter(child => child instanceof HTMLElement && child !== portalHost)
  .map(element => { element.inert = true; element.setAttribute('aria-hidden', 'true'); });

// 3. Required data attributes for gamepad nav (must be present):
// - `data-crosshook-focus-root="modal"` on the surface div
// - `data-crosshook-modal-close` on the close button
// App.tsx handleGamepadBack queries these selectors directly

// 4. Guard before render
if (!open || !isMounted || !portalHostRef.current) return null;
return createPortal(<...>, portalHostRef.current);

// 5. Escape key handling in onKeyDown — stop propagation before close
if (event.key === 'Escape') {
  event.stopPropagation();
  event.preventDefault();
  onClose();
}
```

### 9. App.tsx: Adding the Event Listener + Conditional Render

The event listener pattern (Tauri `listen()` returns `Promise<UnlistenFn>`):

```typescript
// In AppShell or App component:
const [showOnboarding, setShowOnboarding] = useState(false);

useEffect(() => {
  const unlistenPromise = listen<null>('onboarding-check', () => {
    setShowOnboarding(true);
  });
  return () => {
    unlistenPromise.then(unlisten => unlisten());
  };
}, []);

// Conditional render within the component JSX:
{showOnboarding ? <OnboardingWizard onComplete={() => setShowOnboarding(false)} /> : null}
```

---

## Integration Points

### Files to Create (new)

| File                                             | What                                                                                                         |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/onboarding/mod.rs`    | Module root; `pub use checks::check_readiness;`                                                              |
| `crates/crosshook-core/src/onboarding/checks.rs` | `check_readiness() -> Vec<HealthIssue>` composing `discover_steam_root_candidates` + `discover_compat_tools` |
| `src-tauri/src/commands/onboarding.rs`           | `onboarding_check_readiness`, `onboarding_get_status`, `onboarding_mark_complete`                            |
| `src/hooks/useOnboarding.ts`                     | Stage-machine hook mirroring `useInstallGame.ts`                                                             |
| `src/components/OnboardingWizard.tsx`            | Portal wizard mirroring `ProfileReviewModal.tsx`                                                             |
| `src/types/onboarding.ts`                        | `OnboardingStage`, `OnboardingReadinessResult`, `OnboardingStatus`                                           |

### Files to Modify (existing)

| File                                        | Change                                                                                     |
| ------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/lib.rs`          | `pub mod onboarding;`                                                                      |
| `crates/crosshook-core/src/settings/mod.rs` | `onboarding_completed: bool` in `AppSettingsData` + update test `save_and_load_round_trip` |
| `src-tauri/src/commands/mod.rs`             | `pub mod onboarding;`                                                                      |
| `src-tauri/src/lib.rs`                      | Startup event + register 3 commands in `invoke_handler![]`                                 |
| `src/App.tsx`                               | `listen('onboarding-check', ...)` in `useEffect` + conditional `<OnboardingWizard />`      |
| `src/types/index.ts`                        | `export * from './onboarding';`                                                            |

---

## Code Conventions

### Rust

- Command functions: `snake_case` matching `invoke()` call names exactly (verified via type-cast tests)
- Error mapping: extract into `fn map_X_error(e: XError) -> String` helpers (see `settings.rs:7-12`)
- Diagnostics: `&mut Vec<String>` passed to discovery functions; log with `tracing::debug!` after collection
- Module structure: `onboarding/mod.rs` + `onboarding/checks.rs` following existing directory-with-mod-rs pattern

### TypeScript

- Stage types: string union defined in `types/onboarding.ts`; export via `types/index.ts`
- `invoke<T>('command_name', { camelCaseArgs })` — note Tauri serializes Rust `snake_case` params to camelCase in JS
- Error normalization: always use `error instanceof Error ? error.message : String(error)` (same as `normalizeErrorMessage` in `useInstallGame.ts`)
- CSS: use existing `crosshook-modal-*` class namespace; wizard-specific classes use `crosshook-onboarding-*`

---

## Dependencies and Services

### Backend

```
crosshook_core::steam::discovery::discover_steam_root_candidates  →  steam_installed check
crosshook_core::steam::proton::discover_compat_tools              →  proton_available check
crosshook_core::profile::health::{HealthIssue, HealthIssueSeverity}  →  readiness result type
crosshook_core::settings::{AppSettingsData, SettingsStore}        →  onboarding_completed flag
```

### Frontend

```
@tauri-apps/api/core: invoke   →  IPC calls
@tauri-apps/api/event: listen  →  onboarding-check event
types/health.ts: HealthIssue   →  readiness check result type (import directly)
react-dom: createPortal        →  wizard modal portal
```

---

## Gotchas and Warnings

- **Never construct `AppSettingsData` from scratch when saving**: Will silently wipe `community_taps` and `auto_load_last_profile`. Always `load() → mutate field → save()`. This is the most critical gotcha — there is no merge, only full overwrite.

- **`discover_steam_root_candidates` takes an explicit path first**: Pass `""` to skip the configured path and fall back to home-dir defaults. See `startup.rs:128`. Forgetting this and passing `None` won't compile — the parameter is not `Option`.

- **`discover_compat_tools` takes `&[PathBuf]`**: Must chain after `discover_steam_root_candidates`. Cannot call directly with a path string.

- **`HealthIssue.path` must be sanitized**: Apply `sanitize_display_path()` from `commands/shared.rs` before setting `path` in any `HealthIssue` returned over IPC. This strips sensitive home directory paths.

- **350ms startup event races with `auto-load-profile`**: Both fire at 350ms. They are independent — React processes them in arrival order. No coordination needed, but be aware tests may need to account for ordering.

- **Tauri `listen()` returns `Promise<UnlistenFn>`**: The cleanup in `useEffect` must handle the async unlisten. Pattern: `const p = listen(...); return () => { p.then(f => f()); }`. Missing this leaks the listener across hot reloads.

- **Portal host div must not be conditional on `open`**: The host is created unconditionally on mount. Only the content render is gated on `open && isMounted && portalHostRef.current`. Misplacing this guard causes the portal to not mount cleanly.

- **`bool` serde default is `false`**: `onboarding_completed: bool` with struct-level `#[serde(default)]` serializes to TOML only when `true` (TOML omits `false` boolean defaults in pretty-print). Deserializing a file without the key gives `false` — correct behavior for first run.

- **`discover_compat_tools` returns empty vec for not-found**: It does not return an `Err`. Empty `Vec<ProtonInstall>` means no Proton found. The readiness check must interpret `.is_empty()` as the failure case.

- **Test the `settings` round-trip after adding the new field**: The existing `save_and_load_round_trip` test in `settings/mod.rs` must be updated to include `onboarding_completed` in the test struct literal, or it will fail to compile.

---

## Task-Specific Guidance

### Backend: `onboarding/checks.rs`

```rust
use crate::profile::health::{HealthIssue, HealthIssueSeverity};
use crate::steam::discovery::discover_steam_root_candidates;
use crate::steam::proton::discover_compat_tools;

pub fn check_readiness() -> Vec<HealthIssue> {
    let mut issues = Vec::new();
    let mut diagnostics = Vec::new();

    let steam_roots = discover_steam_root_candidates("", &mut diagnostics);

    if steam_roots.is_empty() {
        issues.push(HealthIssue {
            field: "steam_installed".to_string(),
            path: String::new(),
            message: "Steam installation not found on this system.".to_string(),
            remediation: "Install Steam from steampowered.com or via your package manager.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
        return issues;  // No point checking proton without Steam
    }

    let proton_installs = discover_compat_tools(&steam_roots, &mut diagnostics);

    if proton_installs.is_empty() {
        issues.push(HealthIssue {
            field: "proton_available".to_string(),
            path: String::new(),
            message: "No Proton installation found in Steam libraries.".to_string(),
            remediation: "Install Proton via Steam > Settings > Compatibility, or install a GE-Proton release.".to_string(),
            severity: HealthIssueSeverity::Warning,
        });
    }

    for entry in &diagnostics {
        tracing::debug!(entry, "onboarding readiness diagnostic");
    }

    issues
}
```

### Backend: `commands/onboarding.rs` Registration

Three commands go into `invoke_handler![]` in `lib.rs` after the last existing entry:

```rust
commands::onboarding::onboarding_check_readiness,
commands::onboarding::onboarding_get_status,
commands::onboarding::onboarding_mark_complete,
```

### Frontend: Event Listener Placement

Place the `listen('onboarding-check', ...)` in `AppShell` (not `App`) so it has access to `ProfileProvider` context if needed for the empty-state banner check. The `showOnboarding` state lives in `AppShell` alongside `route` state.

### Frontend: Stage Type Definition

Define the `OnboardingStage` type in `src/types/onboarding.ts` and derive all UI text from it in pure functions before the hook — this makes the hook testable and the UI text easy to update without touching hook logic.

### Testing

- Rust: `SettingsStore::with_base_path(tempdir)` injection pattern (see `settings/mod.rs:116`) works for all settings store tests
- Rust: `check_readiness()` is a pure function with no injected state — test by setting up fake Steam dirs via `tempfile::tempdir()` (see `discovery.rs:107-145` for the pattern)
- TypeScript: No test framework configured on frontend — no test files needed for the wizard
