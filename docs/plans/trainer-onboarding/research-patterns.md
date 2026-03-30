# Pattern Research: trainer-onboarding

## Architectural Patterns

**Tauri IPC Command Pattern**: Commands live in `src-tauri/src/commands/<module>.rs`, annotated `#[tauri::command]`. Managed state is injected as `State<'_, StoreType>`. Blocking I/O uses `tauri::async_runtime::spawn_blocking`. All errors are converted to `String` at the boundary. Commands are registered in a flat `tauri::generate_handler![...]` list in `lib.rs`.

- Example (synchronous, State): `src-tauri/src/commands/settings.rs:16-18`
- Example (async, spawn_blocking): `src-tauri/src/commands/install.rs:20-27`
- Registration: `src-tauri/src/lib.rs:123-193`

**Stage-Machine Hook Pattern**: The canonical reference is `useInstallGame.ts`. Key conventions:

- Stage is a string union type: `'idle' | 'preparing' | ... | 'failed'`
- Derived boolean flags exposed on the return value: `isIdle`, `isPreparing`, `hasFailed`, etc.
- Pure helper functions derive computed strings from stage: `deriveStatusText()`, `deriveHintText()`, `deriveActionLabel()`
- Factory functions initialize empty state: `createEmptyRequest()`, `createEmptyValidationState()`
- All setters are `useCallback`-wrapped for referential stability
- Single async driver function (`startInstall`) transitions through stages sequentially
- `reset()` returns all state to its initial values
- Validation error strings mapped to specific form fields via a lookup function

- Full reference: `src/hooks/useInstallGame.ts`

**DiagnosticCollector Pattern**: Builder struct in `steam/diagnostics.rs`. Call `add_diagnostic()` / `add_hint()` to accumulate messages; call `finalize()` to get deduped vecs. Used specifically for Steam discovery flows — not the same as `HealthIssue`. The onboarding `ReadinessCheckResult` reuses `HealthIssue[]` directly, not `DiagnosticCollector`.

- `crates/crosshook-core/src/steam/diagnostics.rs`

**React Context Pattern**: Context wraps a hook (`useProfile`) and adds derived values via `useMemo`. Backend events are subscribed via `listen<T>(event_name, handler)` inside a `useEffect` with cleanup. Consumer hooks throw if called outside the provider.

- `src/context/ProfileContext.tsx`

**Startup Event Pattern**: Backend events are emitted from `tauri::async_runtime::spawn` inside `.setup()`, with a short `sleep` to ensure the frontend is mounted. The onboarding-check event should follow this same pattern.

- `src-tauri/src/lib.rs:59-101`

**Modal Pattern**: `ProfileReviewModal` is the reference. Key conventions:

- Portal-based: creates a host `<div>` appended to `document.body` via `useEffect`
- Sets `aria-hidden`/`inert` on all body children except the portal when open
- Focus trap: Tab/Shift+Tab cycles within `data-crosshook-focus-root="modal"`
- Close button carries `data-crosshook-modal-close` so the gamepad B-button handler in `App.tsx` can find it
- `role="dialog"`, `aria-modal="true"`, `aria-labelledby` pointing to a `useId()`-generated heading ID
- Restores the previously focused element on close

- `src/components/ProfileReviewModal.tsx`

## Code Conventions

**Rust**

- All identifiers: `snake_case`; module directories use `mod.rs` (`crates/crosshook-core/src/<name>/mod.rs`)
- New core module: add `pub mod <name>;` in `crates/crosshook-core/src/lib.rs`
- New command module: add `pub mod <name>;` in `src-tauri/src/commands/mod.rs`, then register all command functions in `src-tauri/src/lib.rs`'s `generate_handler![...]`
- `#[serde(default)]` on the struct **and** every optional field of `AppSettingsData` — guarantees forward compat when a new field is added to `settings.toml`
- Error mapping at IPC boundary: `.map_err(|e| e.to_string())`; intermediate types implement `Display` and use `From` impls for composition
- Synchronous commands take `State<'_, T>` by value; blocking operations need `spawn_blocking`; no `State` needed for pure functions with no side effects

**React / TypeScript**

- Hooks return a typed `UseXxxResult` interface containing all state + all setters
- Type definitions live in `src/types/<domain>.ts` and are re-exported from `src/types/index.ts`
- Components are presentational; hooks own all state
- CSS class names: `crosshook-*` BEM-like (`crosshook-modal__header`, `crosshook-button--ghost`)
- Tauri invoke: `invoke<ReturnType>('command_name', { camelCaseKey: value })` — Tauri auto-converts Rust `snake_case` args to JS `camelCase`
- No `async`/`await` at component level; async logic lives inside hooks' `useCallback`-wrapped functions

**Validation message contract**: TypeScript `INSTALL_GAME_VALIDATION_MESSAGES` constant map mirrors Rust `InstallGameValidationError::message()` exactly. The same pattern must be followed for any onboarding validation errors so `mapValidationErrorToField` can route errors to the correct form field.

- Rust enum: `crates/crosshook-core/src/install/models.rs:63`
- TS mirror: `src/types/install.ts:59-76`

## Error Handling

**IPC Boundary**

- Rust: always `Result<T, String>`; never `Result<T, Box<dyn Error>>`
- Convert with `.map_err(|e| e.to_string())` — one line at the command boundary
- Custom store error types (`SettingsStoreError`) have `Display` impls and are converted via helper `fn map_settings_error(e) -> String`
- Example: `src-tauri/src/commands/settings.rs:7-9`

**Frontend Error Routing**

```typescript
catch (invokeError) {
  const message = normalizeErrorMessage(invokeError); // string, regardless of Error type
  const field = mapValidationErrorToField(message);   // null = general error
  if (field === null) setGeneralError(message);
  else setFieldError(field, message);
}
```

- `normalizeErrorMessage` in `src/hooks/useInstallGame.ts:90-92`
- `mapValidationErrorToField` in `src/hooks/useInstallGame.ts:94-137`

**Validation-before-action pattern**: Call `invoke('validate_<thing>', { ... })` as a separate IPC call before the main action. Throws a descriptive string on failure that frontend can route to a field.

- `src/hooks/useInstallGame.ts:454-456`

## Testing Approach

**Rust (crosshook-core)** — `#[cfg(test)] mod tests` inline in each module file:

- Use `tempfile::tempdir()` for isolated filesystem state
- Use `SettingsStore::with_base_path(dir)` / `ProfileStore::with_base_path(dir)` to inject temp paths
- Test round-trip (save → load), `#[serde(default)]` behavior (partial TOML with missing fields), and edge conditions (blank/whitespace profile names)
- Test naming: `verb_when_condition` (e.g., `returns_none_when_auto_load_is_disabled`)
- Reference: `crates/crosshook-core/src/settings/mod.rs:111-168`, `src-tauri/src/startup.rs:235-317`

**Rust (src-tauri commands)** — compile-time contract tests only:

```rust
#[test]
fn command_names_match_expected_ipc_contract() {
  let _ = settings_load as fn(State<'_, SettingsStore>) -> Result<AppSettingsData, String>;
}
```

This verifies the function signature matches the IPC contract without any runtime behavior.

- Reference: `src-tauri/src/commands/settings.rs:38-51`

**Frontend** — no test framework configured (per CLAUDE.md).

## Patterns to Follow

For `trainer-onboarding`, apply these patterns in order:

1. **Core module**: `crates/crosshook-core/src/onboarding/mod.rs` (types + re-exports) + `readiness.rs` (free function `check_system_readiness()`). Add `pub mod onboarding;` to `crates/crosshook-core/src/lib.rs`.

2. **Settings flag**: Add `#[serde(default)] pub onboarding_completed: bool` to `AppSettingsData` struct in `settings/mod.rs`. The struct-level `#[serde(default)]` already present ensures missing-field compat.

3. **Tauri commands file**: `src-tauri/src/commands/onboarding.rs` — three commands: `check_readiness()` (async + spawn*blocking), `dismiss_onboarding(State<'*, SettingsStore>)`(synchronous, no spawn_blocking),`get_trainer_guidance()`(pure, no State). Add`pub mod onboarding;`to`commands/mod.rs`. Register all three in `lib.rs`.

4. **Startup event**: In `lib.rs::setup()`, after the existing health/version spawns, spawn a task that loads settings and emits `"onboarding-check"` with a `bool` payload (or the full `AppSettingsData`), similar to `"auto-load-profile"` at line 61–70.

5. **Hook**: `src/hooks/useOnboarding.ts` mirrors `useInstallGame.ts`:
   - Stage union: `'readiness_check' | 'trainer_guidance' | 'profile_creation' | 'completed'`
   - Derived booleans: `isReadinessCheck`, `isTrainerGuidance`, `isProfileCreation`, `isCompleted`
   - Factory for initial state; `reset()` function; `useCallback`-wrapped transitions
   - No React Context needed — single consumer (`OnboardingWizard.tsx`)

6. **TypeScript types**: `src/types/onboarding.ts` — import `HealthIssue` from `./health`, define `ReadinessCheckResult` and `OnboardingWizardStage`. Add `export * from './onboarding';` to `src/types/index.ts`.

7. **Modal component**: `src/components/OnboardingWizard.tsx` follows `ProfileReviewModal.tsx` — portal host, `inert`/`aria-hidden` sibling management, focus trap, `data-crosshook-focus-root="modal"`, `data-crosshook-modal-close` on back/close buttons, `role="dialog"`, `aria-modal="true"`.

8. **App.tsx wiring**: Listen for `"onboarding-check"` event with `listen<boolean>()` in a `useEffect` on `AppShell` (or a new `useOnboardingCheck` helper hook). Show `<OnboardingWizard>` when `!onboardingCompleted`.

9. **Capabilities**: No new permissions needed for `check_readiness` / `dismiss_onboarding` / `get_trainer_guidance` — they require no FS shell access beyond what `core:default` already provides.
