# Pattern Research: profile-modal

## Architectural Patterns

**Hook-owned domain state with thin view components**: Stateful workflows live in custom hooks and components mostly render derived state plus callbacks. `useInstallGame` owns install request, stage transitions, candidate derivation, and validation state; `useProfile` owns profile selection, normalization, persistence, and delete flow. `ProfileEditorView` composes those domains rather than duplicating their logic.

- Example: `src/crosshook-native/src/hooks/useInstallGame.ts`
- Example: `src/crosshook-native/src/hooks/useProfile.ts`
- Example: `src/crosshook-native/src/components/ProfileEditor.tsx`

**Parent-owned coordination across feature slices**: Cross-feature handoff is done through parent callbacks and lifted state, not a global store. Today `InstallGamePanel` emits a reviewed profile through `onReviewGeneratedProfile`, and `ProfileEditorView` hydrates the profile hook and switches the subtab. The modal should follow the same pattern by lifting review-session ownership to `ProfileEditorView` and keeping `InstallGamePanel` focused on install execution.

- Example: `src/crosshook-native/src/components/InstallGamePanel.tsx`
- Example: `src/crosshook-native/src/components/ProfileEditor.tsx`
- Example: `src/crosshook-native/src/App.tsx`

**Derived state over duplicated source of truth**: The codebase prefers small pure helpers that derive labels, stages, normalized paths, display names, launch methods, and preview objects from canonical state. This keeps render branches simple and limits mutation points.

- Example: `src/crosshook-native/src/hooks/useInstallGame.ts`
- Example: `src/crosshook-native/src/hooks/useProfile.ts`
- Example: `src/crosshook-native/src/components/InstallGamePanel.tsx`

**Normalize at the domain boundary**: Profiles are normalized when entering edit state and again before persistence. The edit surface works on a predictable `GameProfile` shape rather than scattering fallback logic throughout the UI.

- Example: `src/crosshook-native/src/hooks/useProfile.ts`

**Type modules split by domain ownership**: Frontend contracts are grouped by domain under `src/types`, then re-exported centrally. Install-specific transport models already live in `install.ts`, and shared app imports usually come through `../types` or a domain file. The modal-specific session type belongs in its own file because the repo already separates install transport from profile editing state.

- Example: `src/crosshook-native/src/types/install.ts`
- Example: `src/crosshook-native/src/types/profile.ts`
- Example: `src/crosshook-native/src/types/index.ts`

**Async effects use cancellation guards**: Data-loading effects use an `active` boolean or request id ref to ignore late async results instead of trying to reconcile stale state after unmount or changed inputs.

- Example: `src/crosshook-native/src/components/ProfileEditor.tsx`
- Example: `src/crosshook-native/src/components/InstallGamePanel.tsx`
- Example: `src/crosshook-native/src/hooks/useInstallGame.ts`

**Reusable visual system with local exceptions**: Newer surfaces use shared `crosshook-*` classes and CSS tokens from `theme.css` and `variables.css`. `ProfileEditor.tsx` still contains older inline-style field helpers and a simple fixed overlay for delete confirmation, so modal work should reuse the design tokens and shared classes where possible instead of copying more inline styles.

- Example: `src/crosshook-native/src/styles/theme.css`
- Example: `src/crosshook-native/src/styles/variables.css`
- Example: `src/crosshook-native/src/components/ProfileEditor.tsx`

**No existing modal primitive**: The current profile delete confirmation is an inline fixed overlay without a portal, focus management, or a reusable modal shell. That means `profile-modal` should introduce a real modal primitive instead of pattern-matching the existing delete confirmation too closely.

- Example: `src/crosshook-native/src/components/ProfileEditor.tsx`

## Code Conventions

- React code uses `PascalCase` for components and `camelCase` for helpers, handlers, and hook APIs.
- Types are explicit and local. The codebase uses interfaces, string unions, `Partial<Record<...>>`, and generic key-constrained updaters rather than `any`.
- Tauri IPC is always typed at the callsite with `invoke<T>()` when a response payload is expected.
- State updates favor immutable updater callbacks with object spreads, especially for nested `GameProfile` edits.
- Repeated UI field markup is extracted into local helper components such as `FieldRow`, `InstallField`, and `ProtonPathField` before introducing broader abstractions.
- Derived labels and render conditions live in small pure functions near the component or hook that owns the behavior.
- String inputs are trimmed aggressively at decision boundaries. Empty-string checks gate save/load/selection behavior throughout the profile and install flows.
- Existing code uses `void asyncFn()` inside event handlers and effects when intentionally discarding the returned promise.
- Styles in the newer UI use `crosshook-*` class names plus CSS custom properties for color, spacing, radii, touch target size, and breakpoints.
- Accessibility is present for tabs and labels (`role="tablist"`, `role="tab"`, `aria-selected`, `htmlFor`), but not yet for modal dialogs. A new modal should raise the bar rather than inherit the older overlay pattern.
- `ProfileEditor.tsx` currently mixes inline styles with class-based styles. For `profile-modal`, prefer extracting shared form sections and styling them with the shared theme system so the modal can match the rest of the app and stay responsive.

## Error Handling

- Frontend hooks expose errors as user-displayable `string | null` state, not rich error objects.
- Validation happens before expensive or mutating work. `useProfile.saveProfile()` rejects blank names and missing executable paths before calling Tauri. `useInstallGame.startInstall()` validates the request before launch.
- Install flow separates field errors from general errors. `useInstallGame` maps backend validation messages to specific request keys and stores them in `validation.fieldErrors`; everything else becomes `validation.generalError` or top-level `error`.
- Save/load/delete operations clear stale errors before starting and restore busy flags in `finally` blocks.
- Async load effects catch errors locally and keep the surface usable. Proton install discovery failures clear the install list and expose an inline error instead of breaking the whole view.
- The hooks normalize unknown failures with `err instanceof Error ? err.message : String(err)` instead of swallowing them.
- The one notable exception is `confirmDelete()`, which logs launcher-inspection failures to `console.error` and continues with deletion confirmation. That is an explicit degraded-path decision, not the main pattern.
- Backend-facing boundaries generally convert domain errors into strings for the UI layer.
- For `profile-modal`, follow the existing save boundary: block save until required fields are present, keep modal edits in local state, and on save failure keep the dialog open with the inline error preserved.

## Testing Approach

- The repo currently has backend/unit-test coverage in Rust and no frontend component or hook test harness configured in `src/crosshook-native/package.json`.
- Existing tests are colocated `#[cfg(test)] mod tests` blocks beside the Rust implementation they cover.
- Backend tests focus on pure behavior and filesystem-backed integration at small scope: temp directories, deterministic fixtures, direct assertions on returned models, and explicit edge cases.
- Profile persistence tests exercise save/load/list/delete/rename round trips against a temp store.
- Install tests cover validation specificity, prefix derivation, and discovered executable selection using temp files and a fake executable script.
- Tauri command and startup tests validate boundary behavior separately from the React layer.
- For a `profile-modal` implementation, the most realistic current verification path is:
- Add Rust tests only if backend contracts change, especially around install payload or profile persistence.
- Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` for core behavior.
- Run the frontend type/build check via the existing build script or `npm run build` in `src/crosshook-native` because there is no dedicated frontend test script.
- Do manual UI verification in the Tauri app for the modal-specific behavior: auto-open after install success, reopen from verify, unsaved draft preservation, blocked save when executable is empty, inline save failure handling, explicit post-save tab switch, and viewport behavior at 1280x800 and Steam Deck-like widths.
- If this feature introduces reusable pure helpers on the frontend, keep them easy to unit test later, but do not assume a frontend test runner already exists.

## Patterns to Follow

- Keep install execution in `useInstallGame` and `InstallGamePanel`; do not move install orchestration into the modal.
- Own the modal session in `ProfileEditorView`, alongside the existing install/profile tab coordination, because that component already bridges the two domains.
- Reuse `GameProfile` as the editable draft model and keep the transport/session split explicit: install handoff type in `src/types/install.ts`, modal session type in `src/types/profile-review.ts`.
- Reuse `useProfile` for persistence instead of adding modal-specific save IPC. If needed, extend the hook with a draft-persistence helper rather than duplicating normalization and metadata-sync logic in the modal.
- Extract shared profile form sections out of `ProfileEditor.tsx` so the modal and main profile tab edit the same fields through the same updater shape.
- Preserve the existing immutable nested-update pattern: `updateProfile((current) => ({ ...current, ... }))` or the equivalent modal-draft updater.
- Keep install-critical sections visible and use conditional rendering, not hidden inactive state, for fields that do not apply to the selected launch method.
- Use shared `crosshook-*` classes and CSS variables for layout, spacing, focus states, radii, and breakpoints. Avoid expanding the older inline-style approach.
- Introduce a proper portal-backed dialog shell with explicit accessibility semantics and internal scrolling. The current repo has no reusable modal abstraction, so this feature should establish one cleanly.
- Match existing async safety patterns: cancellation guards for late responses, `finally` for busy flags, and string normalization for error messages.
- Keep success handoff explicit. Existing coordination already changes tabs through parent state; after save, use the same parent-owned navigation pattern to select the saved profile and switch to the Profile tab.
- Verify through typecheck/build plus manual UI behavior, because the current codebase’s automated frontend coverage is effectively build-level, not component-level.
