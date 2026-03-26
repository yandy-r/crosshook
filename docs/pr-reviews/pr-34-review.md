# PR #34 Review: feat(ui): sidebar navigation, page banners, and themed selects

**Branch**: `feat/ui-enhancements` -> `main`
**Reviewed**: 2026-03-26
**Scope**: 5238 additions, 2100 deletions across 42 files
**Agents**: code-reviewer, silent-failure-hunter, type-design-analyzer, comment-analyzer

---

## Critical Issues (3 found)

### 1. Arrow key handlers conflict: `useScrollEnhance` does not check `defaultPrevented`

**Files**: [`useScrollEnhance.ts:66-86`](../../src/crosshook-native/src/hooks/useScrollEnhance.ts), [`useGamepadNav.ts:552-577`](../../src/crosshook-native/src/hooks/useGamepadNav.ts)
**Confidence**: 95%
**Status**: Fixed

`useGamepadNav` registers a keydown handler in the **capture phase** (line 577, third arg `true`) that calls `event.preventDefault()` and moves focus on ArrowUp/Down/Left/Right. `useScrollEnhance` registers a keydown handler in the **bubble phase** (line 90, no third arg) that also acts on arrow keys. Since `preventDefault()` does not stop propagation, both handlers fire: the first moves focus, the second scrolls the container. The scroll handler never checks `e.defaultPrevented`.

Every arrow key press both navigates focus AND scrolls content. The PR test plan item "Arrow keys scroll content when not in text inputs" is marked unchecked, which is likely a symptom of this conflict.

**Fix**: Add `if (e.defaultPrevented) return;` as the first line of `onKeyDown` in `useScrollEnhance.ts`:

```typescript
function onKeyDown(e: KeyboardEvent) {
  if (e.defaultPrevented) return;
  if (isInteractiveTarget(document.activeElement)) return;
  // ... rest of handler
}
```

### 2. Console drawer layout flash on initial render

**File**: [`App.tsx:28-30, 73-81`](../../src/crosshook-native/src/App.tsx)
**Confidence**: 90%
**Status**: Fixed

The console panel is configured with `defaultSize="60%"` and `collapsible`, then collapsed via `useEffect` (which runs **after** paint):

```typescript
useEffect(() => {
  consolePanelRef.current?.collapse();
}, []);
```

On first render, the console drawer occupies 60% of the vertical content area until the effect fires after paint, causing a visible layout jump.

**Fix**: Set `defaultSize` to match the collapsed size so the initial layout is already collapsed:

```tsx
<Panel
  className="crosshook-shell-panel"
  panelRef={consolePanelRef}
  collapsible
  collapsedSize="40px"
  defaultSize="40px"
  minSize="15%"
  maxSize="75%"
>
```

### 3. Silent `.catch(() => undefined)` swallows all errors in SettingsPage

**File**: [`SettingsPage.tsx:48-55`](../../src/crosshook-native/src/components/pages/SettingsPage.tsx)
**Confidence**: 95%
**Status**: Fixed

Three `.catch(() => undefined)` blocks silently swallow every error from `handleAutoLoadChange`, `refreshPreferences`, and `clearRecentFiles`:

```typescript
onAutoLoadLastProfileChange={(enabled) => {
  void handleAutoLoadChange(enabled).catch(() => undefined);
}}
onRefreshRecentFiles={() => {
  void refreshPreferences().catch(() => undefined);
}}
onClearRecentFiles={() => {
  void clearRecentFiles().catch(() => undefined);
}}
```

The `PreferencesContext` functions already set `settingsError` state AND re-throw. The error state propagation works, but the `.catch(() => undefined)` pattern directly violates the CLAUDE.md rule "ALWAYS throw errors early and often. Do not use fallbacks" and trains developers to copy this pattern elsewhere where error state may NOT be set upstream.

**Fix**: Remove the `.catch(() => undefined)` handlers. The `void` prefix already signals the promise is not awaited:

```typescript
onAutoLoadLastProfileChange={(enabled) => {
  void handleAutoLoadChange(enabled);
}}
```

---

## Important Issues (6 found)

### 4. Duplicate `auto-load-profile` event listeners

**Files**: [`ProfileContext.tsx:48`](../../src/crosshook-native/src/context/ProfileContext.tsx), [`PreferencesContext.tsx:109`](../../src/crosshook-native/src/context/PreferencesContext.tsx)
**Confidence**: 85%
**Status**: Fixed

Both contexts register a Tauri event listener for `'auto-load-profile'`. The `ProfileContext` listener calls `selectProfile`. The `PreferencesContext` listener calls `onAutoLoadProfile`, but `AppShell` never passes `onAutoLoadProfile` to `PreferencesProvider` (App.tsx line 33), making the second listener dead code. If `onAutoLoadProfile` is ever passed, both fire on the same event, loading the profile twice.

**Fix**: Remove the `auto-load-profile` listener from `PreferencesContext.tsx` (lines 108-122). `ProfileContext` is the canonical owner of profile selection.

### 5. `useGamepadNav` callbacks invalidated every render due to unstable `options` reference

**File**: [`useGamepadNav.ts:366, 424, 508`](../../src/crosshook-native/src/hooks/useGamepadNav.ts), [`App.tsx:95`](../../src/crosshook-native/src/App.tsx)
**Confidence**: 82%
**Status**: Fixed

Three `useCallback` hooks depend on `[options]`. In `App.tsx:95`, the options object is created inline every render: `useGamepadNav({ onBack: handleGamepadBack })`. The object literal is a new reference each render, causing the gamepad polling `requestAnimationFrame` loop (lines 630-724) to tear down and re-register on every render -- unnecessary GC pressure and potential frame drops.

**Fix**: Memoize the options object in `App.tsx`:

```typescript
const gamepadOptions = useMemo(() => ({ onBack: handleGamepadBack }), []);
const gamepadNav = useGamepadNav(gamepadOptions);
```

### 6. `console.error` without user feedback in LauncherExport

**File**: [`LauncherExport.tsx:128-131`](../../src/crosshook-native/src/components/LauncherExport.tsx)
**Confidence**: 90%
**Status**: Fixed

`refreshLauncherStatus` catches errors, logs to `console.error`, and silently sets `launcherStatus` to `null`. A `null` status means the entire "Exported/Stale/Not Exported" section and the "Delete Launcher" button vanish from the UI with no feedback.

**Fix**: Surface the error in the component's existing `errorMessage` state:

```typescript
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  setErrorMessage(`Failed to check launcher status: ${message}`);
  setLauncherStatus(null);
}
```

### 7. `console.error` without user feedback in useProfile metadata sync

**File**: [`useProfile.ts:404-408`](../../src/crosshook-native/src/hooks/useProfile.ts)
**Confidence**: 90%
**Status**: Fixed

When `syncProfileMetadata` fails (last-used profile, recent files), the error is caught, logged to `console.error`, and silently swallowed. On next launch, auto-load may load the wrong profile or the recent files list will be stale.

**Fix**: Set a non-blocking warning via the hook's existing `setError`:

```typescript
} catch (syncErr) {
  const message = syncErr instanceof Error ? syncErr.message : String(syncErr);
  console.error('Failed to sync profile metadata:', message);
  setError(`Profile loaded, but preferences sync failed: ${message}`);
}
```

### 8. `console.error` without user feedback in PreferencesContext auto-load handler

**File**: [`PreferencesContext.tsx:114-116`](../../src/crosshook-native/src/context/PreferencesContext.tsx)
**Confidence**: 90%
**Status**: Fixed

When the `auto-load-profile` event handler fails, the error is caught and logged with no user notification. The user launches CrossHook expecting their last profile to load, sees an empty editor, and has no indication auto-load was attempted.

**Fix**: Surface through `settingsError`:

```typescript
void Promise.resolve(onAutoLoadProfile(event.payload)).catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  setSettingsError(`Auto-load profile failed for "${event.payload}": ${message}`);
});
```

### 9. Duplicate `resolveLaunchMethod` function

**Files**: [`ProfileContext.tsx:22-38`](../../src/crosshook-native/src/context/ProfileContext.tsx), [`useProfile.ts:94-110`](../../src/crosshook-native/src/hooks/useProfile.ts)
**Confidence**: 85%
**Status**: Fixed

Two independent implementations exist. The `useProfile.ts` version uses the `looksLikeWindowsExecutable()` helper; `ProfileContext.tsx` inlines equivalent logic. They produce the same result, but if resolution logic changes, both must be updated.

**Fix**: Extract to a shared utility (e.g., `utils/launch.ts`) and import in both files.

---

## Suggestions (17 found)

### Type Design Improvements

#### 10. Add exhaustiveness check in `ContentArea.tsx:54`

**Status**: Open

Replace `default: return null` with `const _exhaustive: never = route; return null;` to get a compile error when a new `AppRoute` is added without a corresponding page.

#### 11. Standardize context sentinel to `null`

**Status**: Open

`ProfileContext` uses `undefined` (line 20) while `PreferencesContext` uses `null` (line 34). Standardize on `null` for consistency.

#### 12. Guard the `AppRoute` cast in `App.tsx:37`

**Status**: Open

The `value as AppRoute` downcast bypasses type safety. Add a runtime check with a `Set<string>` lookup before casting.

#### 13. Define a `LogPayload` type for `ConsoleDrawer.tsx:79`

**Status**: Open

Replace `listen<unknown>('launch-log', ...)` with a discriminated union (`string | { line: string } | { message: string } | { text: string }`) to make the event contract explicit.

#### 14. Import `SVGProps` directly in `PageBanner.tsx`

**Status**: Open

Use `SVGProps<SVGSVGElement>` instead of `React.SVGProps<SVGSVGElement>` for consistency with other named imports in the file.

#### 15. ThemedSelect sentinel collision

**Status**: Open

The `EMPTY = '__empty__'` sentinel could collide with actual data values. Consider prefixing with a non-printable character or documenting the constraint via JSDoc on `SelectOption.value`.

### Error Handling Improvements

#### 16. Add `.catch()` on unhandled promise in `ProfileContext.tsx:52`

**Status**: Open

Add `.catch()` on `void profileState.selectProfile(event.payload)` to prevent unhandled promise rejections.

#### 17. PreferencesContext initialization error only visible on Settings page

**Status**: Open

When settings fail to load at startup, only users who navigate to Settings see the error. Consider surfacing critical init errors globally (toast/banner).

#### 18. `removeTap` optimistic state update without rollback (`useCommunityProfiles.ts:208-220`)

**Status**: Open

Local state is updated before `saveSettingsTaps` persists. If save fails, the tap disappears from UI but reappears on next launch. Defer state update until after save succeeds, or add rollback logic.

#### 19. `normalizeLogMessage` returns empty string for unrecognized payload shapes

**Status**: Open

Affects `ConsoleDrawer.tsx:7-31` and `ConsoleView.tsx:18-42`. If the backend changes payload shape, log messages silently disappear. Fallback to `JSON.stringify(payload)` instead.

### Comment & Documentation Improvements

#### 20. Inaccurate comment at `useScrollEnhance.ts:21`

**Status**: Open

`// Radix Select triggers use data-state and aria-expanded` -- the code only checks `aria-expanded`, and it's a general ARIA attribute, not Radix-specific. Rewrite to: `// Elements with aria-expanded (select triggers, disclosure widgets, etc.) manage their own keyboard input`.

#### 21. `normalizeLogMessage` duplicated without cross-reference

**Status**: Open

Identical function in `ConsoleDrawer.tsx:7-31` and `ConsoleView.tsx:18-42`. Add a cross-reference comment, or extract to a shared utility.

#### 22. Missing module-level docs on `PreferencesContext.tsx`

**Status**: Open

193-line file with no comments explaining its relationship to `ProfileContext` (PreferencesContext owns app settings/recent files; ProfileContext owns profile CRUD).

#### 23. Missing module-level docs on `ProfileContext.tsx`

**Status**: Open

The `resolveLaunchMethod` priority cascade (steam_applaunch -> proton_run -> native) is business-critical and undocumented.

#### 24. `useScrollEnhance` constants lack context

**Status**: Open

`WHEEL_MULTIPLIER = 10`, `SMOOTH_FACTOR = 0.18`, `ARROW_SCROLL_PX = 80` are magic numbers. Add a header comment explaining they compensate for WebKitGTK's sluggish scroll velocity.

#### 25. `useGamepadNav` lacks module-level documentation

**Status**: Open

744-line hook with zero comments. The focus zone model (`data-crosshook-focus-zone`, `data-crosshook-focus-root`), gamepad polling, and bumper-based view cycling should be briefly documented.

#### 26. `PageBanner.tsx:27` -- `S` variable name is cryptic

**Status**: Open

The shared SVG props constant should be named `SVG_DEFAULTS` or `ILLUSTRATION_PROPS` for self-documentation.

---

## Strengths

- **Architecture**: Clean context-based decomposition eliminates massive prop drilling from App.tsx (407 -> 84 lines). The sidebar/page/content-area routing is well-layered with `AppRoute` type flowing through all components.
- **Type safety**: Zero `any` types across the entire PR. `ResolvedLaunchMethod = Exclude<LaunchMethod, ''>` is a textbook use of TypeScript type exclusion to enforce a narrower invariant.
- **Context providers**: Both `usePreferencesContext` and `useProfileContext` throw descriptive errors when used outside their providers -- excellent fail-fast behavior. `PreferencesContext` re-throws after setting error state, ensuring both UI and callers can handle errors.
- **ThemedSelect abstraction**: Clean wrapper around Radix Select's empty-string limitation using sentinel mapping. The `hasValue` guard prevents displaying phantom selections after option changes.
- **Exhaustive route mapping**: `Record<AppRoute, string>` for `ROUTE_LABELS` guarantees at compile time that every route has a label. Adding a route without a label is a type error.
- **Cleanup patterns**: Proper `active` flag patterns in all `useEffect` hooks prevent state updates on unmounted components.
- **Accessibility**: SVG illustrations use `aria-hidden`, proper semantic structure throughout, controller prompt hints for Steam Deck.
- **No `any` violations**: Confirmed no `any` types introduced anywhere in the diff, fully compliant with project conventions.

---

## Recommended Action

1. ~~Fix critical issues #1-#3 before merge (arrow-key conflict, console flash, silent catches)~~ **Done**
2. ~~Address important issues #4-#9 (duplicate listeners, unstable refs, silent console.error paths, duplicated function)~~ **Done**
3. Suggestions can be scheduled as follow-up work
