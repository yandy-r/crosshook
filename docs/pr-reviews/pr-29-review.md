# PR #29 Review: feat(profile): add install review modal flow

**PR**: #29 (`feat/profile-modal` -> `main`)
**Date**: 2026-03-25
**Scope**: +4,051 / -634 across 33 files (source: +2,005, docs/plans: +1,981, tasks/docs: +65)
**Closes**: #28

## Overview

This PR adds an in-flow profile review modal to the native install-game workflow. Instead of switching tabs to verify or update a generated profile, the install flow now opens a large review modal, saves through the existing profile pipeline, and hands off to the Profile tab with the saved profile selected. The PR also extracts shared profile form sections into `ProfileFormSections.tsx`, adds dirty-session guardrails, and hardens controller/focus behavior for the modal.

Five specialized review agents analyzed the changes in parallel:

| Agent                 | Focus                                              |
| --------------------- | -------------------------------------------------- |
| Code Reviewer         | CLAUDE.md compliance, bugs, security, code quality |
| Silent Failure Hunter | Swallowed errors, missing logging, race conditions |
| Type Design Analyzer  | Type encapsulation, invariants, type safety        |
| Comment Analyzer      | Comment accuracy, staleness, documentation         |
| Code Simplifier       | Duplication, complexity, re-render risks           |

---

## Critical Issues (2)

### C1. Race condition: `handleSaveProfileReview` uses `requestAnimationFrame` to poll save result

**Status:** Resolved — `persistProfileDraft` returns `PersistProfileDraftResult`; `handleSaveProfileReview` branches on `result.ok` and sets `saveError` from `result.error`. The `profileSaveStateRef` mirror and `requestAnimationFrame` polling were removed.

**Agents**: Code Reviewer (95%), Silent Failure Hunter (HIGH), Code Simplifier
**File**: `src/crosshook-native/src/components/ProfileEditor.tsx:388-405`

After calling `state.persistProfileDraft(draftProfileName, draftProfile)`, the code awaits a `requestAnimationFrame` and then reads `profileSaveStateRef.current.error` to determine success:

```typescript
await state.persistProfileDraft(draftProfileName, draftProfile);
await new Promise<void>((resolve) => {
  window.requestAnimationFrame(() => resolve());
});

if (profileSaveStateRef.current.error) {
  // ...
```

React 18's batched/concurrent updates do not guarantee that `setError` inside `persistProfileDraft`'s catch block will have flushed to the ref by the time one animation frame completes. If timing is off, the modal closes and the draft is discarded even though the save actually failed.

**Impact**: Any backend rejection (disk full, invalid TOML, permission denied) could be missed. The review session is destroyed, the user is switched to the Profile tab, and the draft is gone with no indication of failure and no way to recover.

**Recommended fix**: `persistProfileDraft` should return a success/failure indicator directly (e.g., `Promise<boolean>` or throw on failure) rather than relying on ref-based state polling across animation frames.

---

### C2. `confirmDelete` silently swallows launcher inspection errors

**Status:** Resolved — On `check_launcher_for_profile` failure, the hook calls `setError` with the failure message and returns without opening the delete dialog.

**Agents**: Silent Failure Hunter (CRITICAL)
**File**: `src/crosshook-native/src/hooks/useProfile.ts:402-413`

The `confirmDelete` function catches errors from `check_launcher_for_profile` and falls through to proceed with delete confirmation showing `launcherInfo: null`:

```typescript
} catch (err) {
  console.error('Failed to inspect launcher state before profile delete.', err);
}
setPendingDelete({ name: trimmed, launcherInfo: null });
```

If the launcher check fails (permissions, TOML deserialization, IPC routing), the user sees a delete dialog that omits launcher file information. They unknowingly leave orphaned `.sh` scripts and `.desktop` entries on disk.

**Impact**: Orphaned launcher files persist indefinitely with no user awareness.

**Recommended fix**: Surface the error to the user via `setError()` and return early, or show a warning in the delete dialog that launcher state could not be verified.

---

## Important Issues (7)

### I1. `ProtonPathField` passes the same value to both `error` and `installsError` props

**Status:** Resolved — Steam and `proton_run` `ProtonPathField` call sites pass `error={null}` and `installsError={protonInstallsError}` so list-load errors render once.

**Agent**: Code Reviewer (92%)
**File**: `src/crosshook-native/src/components/ProfileFormSections.tsx:536-537, 642-644`

Both `ProtonPathField` call sites pass `protonInstallsError` to both the `error` and `installsError` props, causing the same error message to render twice. The `error` prop is intended for per-field validation errors and should be `null`.

---

### I2. `deriveSteamClientInstallPath` duplicated in App.tsx and ProfileFormSections.tsx

**Status:** Resolved — `App.tsx` imports `deriveSteamClientInstallPath` from `ProfileFormSections.tsx`.

**Agents**: Code Reviewer (85%), Code Simplifier
**Files**: `src/crosshook-native/src/App.tsx:32-38`, `src/crosshook-native/src/components/ProfileFormSections.tsx:106-112`

Identical implementations. `ProfileEditor.tsx` already imports from `ProfileFormSections`, so `App.tsx` should do the same.

---

### I3. `chooseFile`/`chooseDirectory` duplicated with zero error handling

**Status:** Resolved — Shared [`utils/dialog.ts`](../../src/crosshook-native/src/utils/dialog.ts) wraps `open()`, catches failures, logs, alerts, and returns `null`. `ProfileFormSections` and `InstallGamePanel` import from it.

**Agents**: Code Reviewer (84%), Silent Failure Hunter (MEDIUM), Code Simplifier
**Files**: `src/crosshook-native/src/components/ProfileFormSections.tsx:77-104`, `src/crosshook-native/src/components/InstallGamePanel.tsx:78-105`

Identical Tauri dialog helpers duplicated across both files. Neither copy has error handling -- if the dialog plugin fails, the rejection propagates to `void props.onBrowse?.()` and becomes an unhandled promise rejection. The user sees nothing.

**Recommended fix**: Extract to a shared utility (e.g., `utils/dialog.ts`) with error handling in one place.

---

### I4. `formatProtonInstallLabel` and `ProtonInstallOption` type duplicated

**Agents**: Code Reviewer (83%), Code Simplifier
**Files**: `src/crosshook-native/src/components/ProfileFormSections.tsx:114-121`, `src/crosshook-native/src/components/InstallGamePanel.tsx:19-33`

`formatProtonInstallLabel` is copied verbatim. `InstallGamePanel` also redeclares its own `ProtonInstallOption` type that is structurally identical to the exported one in `ProfileFormSections`.

---

### I5. `syncProfileMetadata` failure blocks and conflates profile load errors

**Status:** Resolved — After a successful `profile_load`, `syncProfileMetadata` runs in its own `try/catch`; failures are `console.error` only and do not clear the loaded profile or reuse the profile-load error path.

**Agent**: Silent Failure Hunter (HIGH)
**File**: `src/crosshook-native/src/hooks/useProfile.ts:215-263`

If `syncProfileMetadata` fails (e.g., `settings_save` rejects), the entire `loadProfile` call is treated as a failure. The user sees an error like "Permission denied: settings.toml" when their actual profile loaded correctly. The metadata sync (last-used profile, recent files) is non-critical and should not block profile loading.

---

### I6. `refreshProfiles` has no internal try/catch; Refresh button produces unhandled rejection

**Status:** Resolved — `refreshProfiles` wraps `profile_list` and follow-up logic in `try/catch` and calls `setError` on failure.

**Agent**: Silent Failure Hunter (HIGH)
**File**: `src/crosshook-native/src/hooks/useProfile.ts:265-284`

`refreshProfiles` calls `invoke('profile_list')` without a try/catch. The "Refresh" button calls `void refreshProfiles()` -- if the profiles directory is inaccessible, this becomes an unhandled promise rejection with no error visible to the user.

---

### I7. Post-delete auto-load failure masquerades as delete failure

**Status:** Resolved — `deleteProfile` / `executeDelete` call `loadProfile` with `loadErrorContext: 'Profile deleted, but loading the next profile failed'` so a failed auto-load is not mistaken for a failed delete.

**Agent**: Silent Failure Hunter (MEDIUM)
**File**: `src/crosshook-native/src/hooks/useProfile.ts:353-391, 418-456`

After deleting a profile, both `deleteProfile` and `executeDelete` auto-select the first remaining profile via `loadProfile(names[0])`. If that load fails (corrupted profile), the catch block sets an error that looks like the delete failed. The user may retry a delete that already succeeded.

---

## Medium Issues (6)

### M1. 11x repeated null-guard `setProfileReviewSession` pattern

**Status:** Resolved — Added `updateProfileReviewSession` helper and routed null-guard updates through it.

**Agents**: Code Simplifier, Type Design Analyzer
**File**: `src/crosshook-native/src/components/ProfileEditor.tsx` (11 call sites)

Every mutation of `profileReviewSession` repeats the same boilerplate:

```typescript
setProfileReviewSession((current) => {
  if (current === null) return current;
  return { ...current /* fields */ };
});
```

A helper function or reducer would eliminate the null-check repetition and centralize invariant enforcement.

---

### M2. `ProfileReviewSession.dirty` is manually tracked instead of derived

**Status:** Resolved — Removed stored `dirty`; derive review dirty state with `useMemo` via `originalProfileName`, `originalProfile`, and `profilesEqual()`.

**Agents**: Type Design Analyzer (encapsulation: 4/10, enforcement: 3/10)
**File**: `src/crosshook-native/src/types/profile-review.ts:13`

`dirty` should reflect whether `draftProfile !== originalProfile`, but it is a manually-managed boolean set in only two handlers (lines 307, 323). A new mutation path that updates `draftProfile` without setting `dirty: true` creates an undetectable inconsistent state.

**Recommended fix**: Compute `dirty` as a derived value via `useMemo` or inside a reducer.

---

### M3. `ProfileFormSectionsProps` profile-selector triad is not type-safe

**Status:** Resolved — Replaced optional triad with a single optional `profileSelector` object (profiles, selectedProfile, onSelectProfile).

**Agent**: Type Design Analyzer (invariant expression: 4/10)
**File**: `src/crosshook-native/src/components/ProfileFormSections.tsx:19-22`

`profiles`, `selectedProfile`, and `onSelectProfile` are three independent optional props that must all be present or all absent. The type allows any 1-of-3 or 2-of-3 combination. A discriminated union would make partial provision a compile error.

---

### M4. `handleGamepadBack` does not account for confirmation sub-dialog layer

**Status:** Resolved — `handleGamepadBack` clicks the last matching modal close control (topmost layer); delete confirmation overlay uses `data-crosshook-focus-root="modal"` and Cancel uses `data-crosshook-modal-close`.

**Agent**: Silent Failure Hunter (MEDIUM)
**File**: `src/crosshook-native/src/App.tsx:70-76`

The gamepad Back button targets `[data-crosshook-modal-close]`, which is only on the outer modal close button. When the confirmation sub-dialog is open inside the modal, Back either does nothing or bypasses the confirmation flow.

---

### M5. `deleteProfile` and `executeDelete` contain duplicated delete-and-refresh logic

**Status:** Resolved — Shared `finalizeProfileDeletion`; removed unused `deleteProfile`; `executeDelete` is the sole delete path.

**Agents**: Code Simplifier, Type Design Analyzer
**File**: `src/crosshook-native/src/hooks/useProfile.ts:353-391 vs 418-456`

Nearly identical implementations. `deleteProfile` also appears to be unused dead code -- `ProfileEditor.tsx` only uses `confirmDelete` and `executeDelete`.

---

### M6. Inline error/warning banner styles duplicated 3x

**Status:** Resolved — Added `.crosshook-error-banner` / `.crosshook-warning-banner` in `theme.css` and replaced inline styles in `ProfileEditor.tsx`.

**Agent**: Code Simplifier
**File**: `src/crosshook-native/src/components/ProfileEditor.tsx:531-542, 606-614, 630-637`

The error banner `{ borderRadius: 12, padding: 12, background: 'rgba(140, 40, 40, 0.2)', ... }` is repeated in three locations. Should be CSS classes (`.crosshook-error-banner`, `.crosshook-warning-banner`).

---

## Low Issues (6)

### L1. `ProfileReviewSession.originalProfile` is never read

**Status:** Resolved — Used for structural equality in derived dirty state (`profilesEqual` vs `draftProfile`).

**Agent**: Code Simplifier
**File**: `src/crosshook-native/src/types/profile-review.ts:8`

Set at construction but never accessed. Likely intended for a "reset to original" feature not yet implemented. Dead weight for now.

---

### L2. `selectProfile` is a trivial wrapper around `loadProfile`

**Status:** Resolved — `selectProfile` is assigned the `loadProfile` function reference (no extra `useCallback`).

**Agent**: Code Simplifier
**File**: `src/crosshook-native/src/hooks/useProfile.ts:286-291`

A `useCallback` that wraps `loadProfile` with zero additional logic.

---

### L3. Module-level `detectedProtonInstalls` constant is misleading

**Status:** Resolved — Removed module constant; `useState<ProtonInstallOption[]>([])`.

**Agent**: Code Reviewer (82%)
**File**: `src/crosshook-native/src/components/InstallGamePanel.tsx:25`

Module-scoped empty array used only as `useState` initial value. Using `[]` inline is clearer and avoids a shared-reference footgun.

---

### L4. Nested ternaries for `reviewDescription` and `statusTone`

**Status:** Resolved — Replaced with an `if` block setting `reviewDescription` and `reviewModalStatusTone`.

**Agent**: Code Simplifier
**File**: `src/crosshook-native/src/components/ProfileEditor.tsx:459-464, 562-568`

Nested ternaries reduce readability. An `if/else` chain would be clearer.

---

### L5. `panelStyle`/`buttonStyle`/`helperStyle` inline objects overlap with CSS classes

**Status:** Resolved — Profile editor shell uses `crosshook-profile-editor-panel`, `crosshook-button` / `crosshook-help-text`, and page classes in `theme.css`.

**Agent**: Code Simplifier
**File**: `src/crosshook-native/src/components/ProfileEditor.tsx:14-42`

Four style constants duplicate what `.crosshook-panel`, `.crosshook-button`, and `.crosshook-help-text` already provide in `theme.css`.

---

### L6. `InstallGameValidationError` union defined but unused

**Status:** Resolved — Added `INSTALL_GAME_VALIDATION_MESSAGES` / `INSTALL_GAME_VALIDATION_FIELD`; `mapValidationErrorToField` matches exact Rust `message()` strings first, then keeps substring fallback.

**Agent**: Type Design Analyzer
**File**: `src/crosshook-native/src/types/install.ts:40-56`

The `mapValidationErrorToField` function at `useInstallGame.ts:91-127` does raw substring matching on error message strings instead of switching on this carefully defined union type.

---

## Documentation Gaps

The comment analyzer found that **none of the six modified source files contain a single code comment**. While the code is generally clean and self-documenting, several complex patterns would benefit from brief explanatory comments:

| Location                         | What needs a comment                                                                                            |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| _(removed)_                      | Previously `profileSaveStateRef` — replaced by `PersistProfileDraftResult` from `persistProfileDraft` (C1 fix). |
| `ProfileEditor.tsx:121-174`      | The promise-resolver confirmation handshake pattern                                                             |
| `ProfileEditor.tsx:176-253`      | The three branches of `handleOpenProfileReview` and the `sameReviewResult` heuristic                            |
| `ProfileReviewModal.tsx:166-233` | Modal accessibility lifecycle (inert, aria-hidden, focus trap, scroll lock)                                     |
| `ProfileReviewModal.tsx:235-278` | Focus trap scoping between confirmation overlay and main surface                                                |
| `useProfile.ts:77-93`            | The `resolveLaunchMethod` fallback chain heuristic                                                              |
| `useProfile.ts:95-149`           | Relationship between `normalizeProfileForEdit` and `normalizeProfileForSave`                                    |
| `useGamepadNav.ts:102-132`       | The multi-signal Steam Deck detection heuristic and known limitations                                           |
| `useGamepadNav.ts:379-453`       | Gamepad polling loop (edge detection, analog threshold, stale cleanup)                                          |
| `ProfileFormSections.tsx:58-75`  | Auto-derive working directory from executable path logic                                                        |

Additional note: The user-facing text at `ProfileEditor.tsx:736` says "save it to Tauri storage" -- profiles are actually saved as TOML files on disk via Tauri IPC, not through Tauri's storage plugin. Consider "save it to disk" for accuracy.

---

## Divergent Constants

The `FOCUSABLE_SELECTOR` constants in `ProfileReviewModal.tsx:46` and `useGamepadNav.ts:26` differ in meaningful ways (the modal version excludes hidden inputs and omits `summary`; the gamepad version uses broader `[href]` instead of `a[href]`). If these differences are intentional they should be documented; if not, this is a latent inconsistency.

---

## Strengths

- **Well-structured extraction**: `ProfileFormSections` cleanly pulls shared form logic out of the monolithic `ProfileEditor.tsx`, reducing it from ~1,000+ lines.
- **Thorough accessibility**: `ProfileReviewModal` has proper focus trapping, `aria-modal`, `aria-labelledby`, `aria-describedby`, `role="dialog"` and `role="alertdialog"` for the confirmation overlay.
- **Zero `any` types**: Strict TypeScript throughout -- no violations of the project's `any` prohibition.
- **Clean type definitions**: `ProfileReviewSource` union, `ProfileReviewModalStatusTone`, and `InstallProfileReviewPayload` follow the project's domain separation patterns.
- **CSS conventions**: New styles follow the `crosshook-*` BEM-like naming convention consistently.
- **Active guard pattern**: The `let active = true` cleanup pattern in effects correctly prevents state updates on unmounted components.
- **Dirty-draft guardrails**: The confirmation flow for discarding unsaved drafts is thorough, covering new-install-arriving, close-attempt, retry, and reset scenarios.
- **Validation before save**: `handleSaveProfileReview` validates both profile name and executable path with specific user-facing messages.

---

## Recommended Action

1. ~~**Fix C1 and C2 first**~~ **Done** — explicit persist result and launcher-check errors surfaced (see Critical Issues above).
2. ~~**Address I1-I4**~~ **Done** — see Important Issues above (Proton field errors, shared `deriveSteamClientInstallPath`, `utils/dialog`, shared Proton install helpers).
3. ~~**Consider I5-I7**~~ **Done** — metadata sync is non-blocking; `refreshProfiles` surfaces errors; post-delete load errors are prefixed (see Important Issues above).
4. **M1-M6 and L1-L6** are cleanup items that can be addressed in a follow-up refactoring pass.
5. **Add comments** to the 10 locations identified in the Documentation Gaps section.
