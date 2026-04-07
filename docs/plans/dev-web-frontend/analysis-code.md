# Code Analysis: dev-web-frontend

## Executive Summary

The CrossHook frontend currently imports Tauri APIs in exactly 42 files (`@tauri-apps/api/core`), 13 files (`@tauri-apps/api/event`), and 6 files (`@tauri-apps/plugin-*`), none of which resolve outside a Tauri WebView. The adapter layer in `src/crosshook-native/src/lib/` will slot in as a transparent drop-in: `callCommand` replaces `invoke`, `subscribeEvent` replaces `listen`, and plugin-stub modules replace the three plugin packages at their import paths — every call site gains the benefit without any change to component or hook logic. The most critical integration points are the parallel 3-command boot in `PreferencesContext.tsx` (lines 43–46), the synchronous `convertFileSrc` usage in two files, and the `ProfileSummary` type that is currently local to `useLibrarySummaries.ts` and must be promoted to `types/library.ts` before mock handlers can import it.

---

## Existing Code Structure

### Tauri API Usage (current state, grep counts)

- `@tauri-apps/api/core` (`invoke`, `convertFileSrc`): **42 files**
  - `src/crosshook-native/src/context/PreferencesContext.tsx` — 4 `invoke` calls (parallel boot + save + steamgriddb key)
  - `src/crosshook-native/src/hooks/useProfile.ts` — largest hook (55 KB), many `invoke` calls
  - `src/crosshook-native/src/hooks/useInstallGame.ts` — `install_game`, `validate_install_request`, `install_default_prefix_path`
  - `src/crosshook-native/src/hooks/useLaunchState.ts` — `invoke` + `listen` combined
  - `src/crosshook-native/src/hooks/useGameCoverArt.ts` — uses `convertFileSrc` synchronously in a `useMemo`
  - `src/crosshook-native/src/components/profile-sections/MediaSection.tsx` — uses `convertFileSrc` synchronously in a callback
  - `src/crosshook-native/src/utils/optimization-catalog.ts` — non-hook, non-component module-level cache

- `@tauri-apps/api/event` (`listen`): **13 files**
  - `src/crosshook-native/src/App.tsx` — `listen<OnboardingCheckPayload>('onboarding-check', ...)` at line 67
  - `src/crosshook-native/src/context/ProfileContext.tsx` — `listen<string>('auto-load-profile', ...)` in `useEffect`
  - `src/crosshook-native/src/hooks/useProfile.ts` — multiple event subscriptions
  - `src/crosshook-native/src/hooks/useLaunchState.ts` — launch event listeners
  - `src/crosshook-native/src/hooks/useProfileHealth.ts` — health event listeners
  - `src/crosshook-native/src/hooks/useRunExecutable.ts` — run-executable event listeners
  - `src/crosshook-native/src/hooks/useCommunityProfiles.ts` — community event listeners
  - `src/crosshook-native/src/hooks/useUpdateGame.ts` — update-log listeners
  - `src/crosshook-native/src/components/ConsoleView.tsx` — `launch-log` and `update-log` listeners
  - `src/crosshook-native/src/components/layout/ConsoleDrawer.tsx` — console event listeners
  - `src/crosshook-native/src/components/pages/LaunchPage.tsx` — page-level event listening
  - `src/crosshook-native/src/hooks/useOfflineReadiness.ts` — offline readiness events
  - `src/crosshook-native/src/components/PrefixDepsPanel.tsx` — prefix-dep-complete event

- `@tauri-apps/plugin-dialog`: **2 files**
  - `src/crosshook-native/src/utils/dialog.ts` — wraps `open` and `save` into `chooseFile`, `chooseSaveFile`, `chooseDirectory`
  - `src/crosshook-native/src/components/CommunityBrowser.tsx` — direct `open()` call

- `@tauri-apps/plugin-shell`: **4 files**
  - `src/crosshook-native/src/components/ExternalResultsSection.tsx`
  - `src/crosshook-native/src/components/ProtonDbLookupCard.tsx`
  - `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`
  - `src/crosshook-native/src/components/SettingsPanel.tsx`
  - All use `open(url)` to open URLs in the system browser

- `@tauri-apps/plugin-fs`: **0 files** (no direct `plugin-fs` imports found in `src/`; `plugin-fs` is listed in `package.json` dependencies but appears unused directly in TypeScript — the stub still needs to be created per spec for completeness)

- Non-hook invoke: `src/crosshook-native/src/utils/optimization-catalog.ts` — standalone module-level async cache function `fetchOptimizationCatalog()` at line 34

- Direct `listen` in `App.tsx`: line 67 (`onboarding-check` event) inside `AppShell` `useEffect`

### File Organization Pattern

The `src/crosshook-native/src/` directory follows a flat feature-domain layout:

```
src/
├── App.tsx              — root shell (listen import at line 5)
├── main.tsx             — React entry point (CSS imports, ReactDOM.createRoot)
├── vite-env.d.ts        — Vite env type declarations (no __WEB_DEV_MODE__ yet)
├── components/          — UI components; subdirs: icons/, install/, layout/, library/, pages/, profile-sections/, ui/, wizard/
├── context/             — React context providers (PreferencesContext, ProfileContext, LaunchStateContext, ProfileHealthContext)
├── hooks/               — All custom hooks (34 files, many wrapping invoke)
├── platform/            — Platform-specific utilities
├── styles/              — CSS files; variables.css, theme.css, layout.css, etc.
├── types/               — TypeScript type definitions (25 files + index.ts barrel)
└── utils/               — Utility modules including optimization-catalog.ts and dialog.ts
```

The new `lib/` directory will slot in as a peer of `components/`, `hooks/`, `context/`, etc. It introduces a clean architectural boundary that did not previously exist: no module currently owns the Tauri API abstraction.

**Key observation**: `types/library.ts` exists but is **not re-exported** from `types/index.ts`. All other domain type files are included in the barrel. This omission means `ProfileSummary` (after it is added to `types/library.ts`) must also be wired into `types/index.ts` so mock handlers can import it via the barrel.

### Import Convention

- All imports use relative paths (e.g., `'../types'`, `'../../utils/dialog'`). No path alias (`@/`) exists yet — Vite's `resolve.alias` and `tsconfig.json` `paths` do not currently define `@`.
- Type-only imports use the `import type { ... }` form consistently (e.g., `import type { AppSettingsData, RecentFilesData } from '../types';`).
- Named exports are the norm; default exports used only for page components.
- After Phase 1, `@/lib/ipc`, `@/lib/events`, and `@/lib/plugin-stubs/*` will be the canonical import paths using the new `@` alias — all 42 core + 13 event files get the new alias form.

---

## Implementation Patterns

### Pattern: React Context Provider with IPC Boot

**Description**: Context providers use a standalone `async function load...()` that calls `Promise.all([invoke<T>(...), ...])` for parallel boot. The result is applied via a `useCallback` inside a `useEffect` that declares an `active` flag to guard against unmount races. The `active` flag is set to `false` in the cleanup function; state setters are only called when `active === true`.

**Example**: See `src/crosshook-native/src/context/PreferencesContext.tsx` lines 42–101:

- `loadPreferences()` at lines 42–54 is the extracted async helper — contains `Promise.all([invoke<AppSettingsData>('settings_load'), invoke<RecentFilesData>('recent_files_load'), invoke<string>('default_steam_client_install_path')])`.
- The mount `useEffect` at lines 78–101 declares `let active = true`, calls `void initializePreferences()`, and returns `() => { active = false; }`.
- Error narrowing uses the local `formatError(error: unknown): string` helper at line 38 — `error instanceof Error ? error.message : String(error)`.

**Apply to**: Migration replaces each `invoke(` with `callCommand(` and updates the import. The `async` structure, `active` flag, `Promise.all`, and `formatError` pattern must be preserved exactly. The parallel boot at lines 43–46 is the single most critical migration site — if mock handlers do not resolve `settings_load`, `recent_files_load`, and `default_steam_client_install_path` synchronously, the entire app shell fails to render.

### Pattern: Custom Hook Wrapping invoke

**Description**: Hooks use `useCallback` to wrap the async `invoke<T>` call with `try/catch/finally`. The error branch calls `console.error(...)` then `setError(String(err))`. The finally block calls `setLoading(false)`. A `useEffect` with relevant dependencies calls `void fetchXxx()`.

**Example**: See `src/crosshook-native/src/hooks/useLibrarySummaries.ts` lines 40–65:

```ts
const fetchSummaries = useCallback(async () => {
  try {
    const result = await invoke<ProfileSummary[]>('profile_list_summaries');
    // ... transform and setState
  } catch (err) {
    console.error('Failed to fetch profile summaries', err);
    setError(String(err));
  } finally {
    setLoading(false);
  }
}, []);

useEffect(() => {
  void fetchSummaries();
}, [profiles, fetchSummaries]);
```

**Apply to**: Migration is mechanical — replace `invoke<T>` with `callCommand<T>`, update the import line. The `try/catch/finally`, `console.error`, `setError(String(err))` pattern is uniform across all 34 hook files and must not be altered. Hooks that import both `invoke` and `listen` (e.g., `useProfile.ts`, `useLaunchState.ts`) require both replacements in the same PR.

### Pattern: Direct invoke in Component (Architectural Smell)

**Description**: 5 component files call `invoke()` directly without delegating to a hook. This is an existing architectural smell documented in `research-practices.md`. The components are: `ProfileActions.tsx`, `TrainerDiscoveryPanel.tsx`, `LaunchPanel.tsx`, `pages/LaunchPage.tsx`, `pages/HealthDashboardPage.tsx`.

**Example**: `src/crosshook-native/src/components/pages/LaunchPage.tsx` line 37 — `invoke<boolean>('check_gamescope_session')` called inline in an event handler.

**Apply to**: Mechanical find/replace only. Do NOT refactor these to hooks in this plan; the follow-up `refactor:` issue is out of scope. The migration changes `import { invoke }` to `import { callCommand } from '@/lib/ipc'` and `invoke(` to `callCommand(` — nothing else.

### Pattern: listen() cleanup in useEffect

**Description**: All `listen()` calls follow a consistent cleanup pattern: the `listen()` call returns a `Promise<UnlistenFn>`. The cleanup stores the promise in a variable and calls `.then(f => f())` or `void unlistenPromise.then(unlisten => unlisten())` in the useEffect return function.

**Two cleanup variants observed in the codebase:**

Variant A (App.tsx line 70–72, most common in hooks):

```ts
const p = listen<OnboardingCheckPayload>('onboarding-check', handler);
return () => {
  p.then((f) => f());
};
```

Variant B (ProfileContext.tsx line 44–47, with active guard):

```ts
const unlistenPromise = listen<string>('auto-load-profile', handler);
return () => {
  active = false;
  void unlistenPromise.then((unlisten) => unlisten());
};
```

Variant C (ConsoleView.tsx lines 65–72, multiple listeners):

```ts
const unlistenLaunch = listen<LogPayload>('launch-log', handler);
const unlistenUpdate = listen<LogPayload>('update-log', handler);
return () => {
  active = false;
  void unlistenLaunch.then((unlisten) => unlisten());
  void unlistenUpdate.then((unlisten) => unlisten());
};
```

**Apply to**: `subscribeEvent` MUST return `Promise<UnlistenFn>` with the same type signature as `listen` from `@tauri-apps/api/event`. The existing cleanup code (`p.then((f) => f())`, `.then(unlisten => unlisten())`) must continue to work without modification. The adapter's browser-mode implementation returns a synchronous unsubscribe function wrapped as `() => void browserBus.get(name)?.delete(wrapped)`, which satisfies the `UnlistenFn = () => void` type.

### Pattern: CSS Variables + BEM Classes

**Description**: All design tokens are CSS custom properties defined in `variables.css`. Component CSS uses `crosshook-*` prefixed class names in BEM-like form. Status chips use the base class `.crosshook-status-chip` with modifier classes appended.

**Token for chip/outline**: `--crosshook-color-warning: #f5c542` (line 17 in `variables.css`). Note: the `.crosshook-status-chip--warning` modifier in `theme.css` (lines 5423–5427) uses `#d97706` (amber-600) for color, not the `--crosshook-color-warning` variable directly. The modifier definition is:

```css
.crosshook-status-chip--warning {
  background: rgba(217, 119, 6, 0.12);
  border-color: rgba(217, 119, 6, 0.28);
  color: #d97706;
}
```

**`.crosshook-app` root**: Defined at `theme.css` line 53 — sets `min-height: 100dvh`, padding, background gradient. The `.crosshook-app--webdev` modifier applied for `box-shadow: inset 0 0 0 3px var(--crosshook-color-warning)` adds a warning-colored inset border over the existing gradient background with zero layout impact.

**Apply to**: `<DevModeBanner />` (or `<DevModeChip />`) reuses `crosshook-status-chip crosshook-status-chip--warning` as its base classes and adds a new `crosshook-dev-chip` modifier class for fixed positioning. The inset outline uses `--crosshook-color-warning` (#f5c542) not the chip-modifier's hardcoded amber value, to distinguish the outline color from the chip color — both are visually warm-yellow but intentionally distinct per the UX two-layer spec.

---

## Integration Points

### Files to Create (Phase 1)

- `src/crosshook-native/src/lib/runtime.ts` — `isTauri()` probe via `(globalThis as Record<string, unknown>).isTauri`; zero deps, no DOM
- `src/crosshook-native/src/lib/ipc.ts` — `callCommand<T>` adapter with `ensureMocks()` + `__WEB_DEV_MODE__` guard + dynamic `import('./mocks')`
- `src/crosshook-native/src/lib/events.ts` — `subscribeEvent<T>` + `emitMockEvent` + module-scope `browserBus: Map<string, Set<Listener>>`
- `src/crosshook-native/src/lib/plugin-stubs/dialog.ts` — real `@tauri-apps/plugin-dialog` re-export in Tauri; `null` + `console.warn('[dev-mock] dialog suppressed')` in browser
- `src/crosshook-native/src/lib/plugin-stubs/shell.ts` — `open(url)` logs a warn and is a no-op in browser; `execute` throws
- `src/crosshook-native/src/lib/plugin-stubs/fs.ts` — `writeFile`/`removeFile`/`rename` throw; `readFile` returns stub data
- `src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts` — `isTauri()` ? real `convertFileSrc` from `@tauri-apps/api/core` : `(path: string) => path` passthrough
- `src/crosshook-native/src/lib/mocks/index.ts` — `registerMocks(): Map<string, Handler>` orchestrator
- `src/crosshook-native/src/lib/mocks/store.ts` — `MockStore` type + `getStore()` singleton (initialized from `DEFAULT_APP_SETTINGS`, `createDefaultProfile()`)
- `src/crosshook-native/src/lib/mocks/eventBus.ts` — re-exports `emitMockEvent` from `lib/events.ts` for use inside handlers
- `src/crosshook-native/src/lib/mocks/README.md` — contributor guide: how to add a handler, fixture content policy, HMR reset behavior
- `src/crosshook-native/src/lib/mocks/handlers/settings.ts` — `settings_load`, `settings_save`, `recent_files_load`, `recent_files_save`, `default_steam_client_install_path`, `settings_save_steamgriddb_key`
- `src/crosshook-native/src/lib/mocks/handlers/profile.ts` — `profile_list_summaries`, `profile_load`, `profile_save`, `profile_duplicate`, `profile_rename`, `profile_list_favorites`, `profile_delete`
- `src/crosshook-native/src/lib/DevModeBanner.tsx` — Layer 2 fixed-position corner chip; renders only when `__WEB_DEV_MODE__` is true
- `src/crosshook-native/src/lib/dev-indicator.css` — `.crosshook-app--webdev { box-shadow: inset 0 0 0 3px var(--crosshook-color-warning); }` + chip sizing; imported conditionally in App.tsx

### Files to Modify (Phase 1)

- `src/crosshook-native/vite.config.ts` — add `({ mode }) =>` wrapper, `define: { __WEB_DEV_MODE__: mode === 'webdev' }`, `resolve: { alias: { '@': './src' } }`, webdev-mode `server.host = '127.0.0.1'` (currently `host: host || false`)
- `src/crosshook-native/package.json` — add `"dev:browser": "vite --mode webdev"` to the `scripts` block (currently has `dev`, `build`, `preview`, `tauri`)
- `src/crosshook-native/tsconfig.json` — add `"paths": { "@/*": ["./src/*"] }` inside `compilerOptions` (currently has no `paths` key)
- `src/crosshook-native/src/vite-env.d.ts` — add `declare const __WEB_DEV_MODE__: boolean;` below the existing `ImportMeta` interface declaration
- `src/crosshook-native/src/App.tsx` — change `import { listen }` to `import { subscribeEvent }` from `@/lib/events`; add `__WEB_DEV_MODE__` conditional `className` on the `<main>` at line 147; add `<DevModeBanner />` render before `<ProfileProvider>`; migrate line 67 `listen(` to `subscribeEvent(`
- `src/crosshook-native/src/main.tsx` — add `import '@/lib/plugin-stubs/convertFileSrc'` as eager side-effect import (or re-export) so the synchronous `convertFileSrc` stub is initialized before any render
- `src/crosshook-native/src/types/library.ts` — add `export interface ProfileSummary { name: string; gameName: string; steamAppId: string; customCoverArtPath?: string; customPortraitArtPath?: string; }` (currently only has `LibraryViewMode` and `LibraryCardData`)
- `src/crosshook-native/src/types/index.ts` — add `export * from './library';` (currently missing from the barrel)
- `src/crosshook-native/src/hooks/useLibrarySummaries.ts` — remove the local `interface ProfileSummary` (lines 6–12); add `import type { ProfileSummary } from '../types/library'`; migrate `invoke<ProfileSummary[]>(` to `callCommand<ProfileSummary[]>(`
- `src/crosshook-native/src/context/PreferencesContext.tsx` — migrate 4 `invoke(` calls: `loadPreferences()` helper (3 in `Promise.all`), `persistSettings` (2: save + reload), `handleSteamGridDbApiKeyChange` (1), `clearRecentFiles` (1); total 7 invoke call sites in this file
- `src/crosshook-native/src/utils/optimization-catalog.ts` — migrate `invoke<OptimizationCatalogPayload>('get_optimization_catalog')` to `callCommand<OptimizationCatalogPayload>('get_optimization_catalog')`
- `scripts/dev-native.sh` — add `--browser|--web)` case branch before the `--help|-h)` branch (spec shows the exact bash structure; the existing `case` is at line 16)
- `.github/workflows/release.yml` — add `verify:no-mocks` step after the "Build native AppImage" step (line 103); uses the grep pattern from spec
- `AGENTS.md` — add `./scripts/dev-native.sh --browser` to the Commands block at line 55; add note about loopback-only binding

**All 42 files containing `import { invoke } from '@tauri-apps/api/core'` — mechanical rewrite:**

```
src/crosshook-native/src/components/AutoPopulate.tsx
src/crosshook-native/src/components/CommunityImportWizardModal.tsx
src/crosshook-native/src/components/LauncherExport.tsx
src/crosshook-native/src/components/LaunchPanel.tsx
src/crosshook-native/src/components/OnboardingWizard.tsx
src/crosshook-native/src/components/pages/HealthDashboardPage.tsx
src/crosshook-native/src/components/pages/LaunchPage.tsx
src/crosshook-native/src/components/pages/ProfilesPage.tsx
src/crosshook-native/src/components/ProfileActions.tsx
src/crosshook-native/src/components/profile-sections/MediaSection.tsx
src/crosshook-native/src/components/SettingsPanel.tsx
src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx
src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx
src/crosshook-native/src/context/PreferencesContext.tsx
src/crosshook-native/src/hooks/useCommunityProfiles.ts
src/crosshook-native/src/hooks/useExternalTrainerSearch.ts
src/crosshook-native/src/hooks/useGameCoverArt.ts
src/crosshook-native/src/hooks/useGameDetailsProfile.ts
src/crosshook-native/src/hooks/useGameMetadata.ts
src/crosshook-native/src/hooks/useInstallGame.ts
src/crosshook-native/src/hooks/useLauncherManagement.ts
src/crosshook-native/src/hooks/useLaunchState.ts
src/crosshook-native/src/hooks/useLibrarySummaries.ts
src/crosshook-native/src/hooks/useMangoHudPresets.ts
src/crosshook-native/src/hooks/useOfflineReadiness.ts
src/crosshook-native/src/hooks/useOnboarding.ts
src/crosshook-native/src/hooks/usePrefixDeps.ts
src/crosshook-native/src/hooks/usePrefixStorageManagement.ts
src/crosshook-native/src/hooks/usePreviewState.ts
src/crosshook-native/src/hooks/useProfileHealth.ts
src/crosshook-native/src/hooks/useProfile.ts
src/crosshook-native/src/hooks/useProtonDbLookup.ts
src/crosshook-native/src/hooks/useProtonDbSuggestions.ts
src/crosshook-native/src/hooks/useProtonInstalls.ts
src/crosshook-native/src/hooks/useProtonMigration.ts
src/crosshook-native/src/hooks/useProtonUp.ts
src/crosshook-native/src/hooks/useRunExecutable.ts
src/crosshook-native/src/hooks/useSetTrainerVersion.ts
src/crosshook-native/src/hooks/useTrainerDiscovery.ts
src/crosshook-native/src/hooks/useTrainerTypeCatalog.ts
src/crosshook-native/src/hooks/useUpdateGame.ts
src/crosshook-native/src/utils/optimization-catalog.ts
```

**Migration transform per file (invoke):**

```
Before: import { invoke } from '@tauri-apps/api/core';
After:  import { callCommand } from '@/lib/ipc';

Before: await invoke<T>('command_name', args)
After:  await callCommand<T>('command_name', args)
```

Files that also import `convertFileSrc` from `@tauri-apps/api/core` (`useGameCoverArt.ts`, `MediaSection.tsx`) need a split import:

```
Before: import { convertFileSrc, invoke } from '@tauri-apps/api/core';
After:  import { convertFileSrc } from '@/lib/plugin-stubs/convertFileSrc';
        import { callCommand } from '@/lib/ipc';
```

**All 13 files containing `import { listen } from '@tauri-apps/api/event'` — mechanical rewrite:**

```
src/crosshook-native/src/App.tsx
src/crosshook-native/src/components/ConsoleView.tsx
src/crosshook-native/src/components/layout/ConsoleDrawer.tsx
src/crosshook-native/src/components/pages/LaunchPage.tsx
src/crosshook-native/src/components/PrefixDepsPanel.tsx
src/crosshook-native/src/context/ProfileContext.tsx
src/crosshook-native/src/hooks/useCommunityProfiles.ts
src/crosshook-native/src/hooks/useLaunchState.ts
src/crosshook-native/src/hooks/useOfflineReadiness.ts
src/crosshook-native/src/hooks/useProfileHealth.ts
src/crosshook-native/src/hooks/useProfile.ts
src/crosshook-native/src/hooks/useRunExecutable.ts
src/crosshook-native/src/hooks/useUpdateGame.ts
```

**Migration transform per file (listen):**

```
Before: import { listen } from '@tauri-apps/api/event';
After:  import { subscribeEvent } from '@/lib/events';

Before: listen<T>('event-name', handler)
After:  subscribeEvent<T>('event-name', handler)
```

Some files (`useLaunchState.ts`, `useProfile.ts`, `useCommunityProfiles.ts`, `useOfflineReadiness.ts`) import both `invoke` from core AND `listen` from event — both imports must be updated in the same edit.

**All 6 files containing `@tauri-apps/plugin-*` — rewrite import paths:**

```
src/crosshook-native/src/utils/dialog.ts           → '@/lib/plugin-stubs/dialog'
src/crosshook-native/src/components/CommunityBrowser.tsx      → '@/lib/plugin-stubs/dialog'
src/crosshook-native/src/components/ExternalResultsSection.tsx → '@/lib/plugin-stubs/shell'
src/crosshook-native/src/components/ProtonDbLookupCard.tsx     → '@/lib/plugin-stubs/shell'
src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx  → '@/lib/plugin-stubs/shell'
src/crosshook-native/src/components/SettingsPanel.tsx          → '@/lib/plugin-stubs/shell' (and '@/lib/plugin-stubs/dialog' if it uses both)
```

Note: No file directly imports `@tauri-apps/plugin-fs` from `src/` — the `plugin-fs` stub must still be created for completeness (spec decision D4) but there are no call sites to migrate.

---

## Code Conventions

### Naming

- `.tsx` for React components in PascalCase (`DevModeBanner.tsx`, `DevModeChip.tsx` — either name is acceptable per spec)
- `.ts` for utilities, hooks, and types in camelCase (`runtime.ts`, `ipc.ts`, `events.ts`)
- CSS classes: `kebab-case` with `crosshook-*` prefix; BEM modifiers with `--` (`crosshook-status-chip--warning`, `crosshook-app--webdev`, `crosshook-dev-chip`)
- TypeScript interfaces: `PascalCase`, exported from `types/` and re-exported via `types/index.ts` barrel
- Constants: `UPPER_SNAKE_CASE` (e.g., `EMPTY_RECENT_FILES`, `DEFAULT_APP_SETTINGS`, `SCROLLABLE`)

### Error Handling

- **Hooks**: `try { await callCommand<T>(...) } catch (err) { console.error('Descriptive message', err); setError(String(err)); } finally { setLoading(false); }` — the `String(err)` conversion is consistent across all hooks; do not change to `err instanceof Error ? err.message : String(err)` unless the hook already uses that form.
- **Contexts**: Use a `formatError(error: unknown): string` helper defined locally in each context file — `error instanceof Error ? error.message : String(error)`. `PreferencesContext.tsx` defines this at line 38.
- **New adapter**: `callCommand` throws `new Error('[dev-mock] Unhandled command: ${name}. Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md')` for unregistered commands. This propagates through the existing `catch (err) { setError(String(err)) }` paths without any change to calling code.
- **Plugin stubs**: Dialog returns `null` with `console.warn('[dev-mock] dialog suppressed in browser mode')`. Shell `open(url)` logs a warn. Destructive operations (`shell.execute`, `fs.writeFile`, `fs.removeFile`, `fs.rename`) throw — no silent no-ops.

### Testing

There is **no configured frontend test framework**. The verification command is:

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

This tests the Rust backend only. Frontend correctness is verified by:

1. Running `./scripts/dev-native.sh --browser` and visually inspecting the UI at `http://localhost:5173`
2. Running `./scripts/dev-native.sh` (full Tauri dev) to confirm the real Tauri path is unaffected
3. Running `./scripts/build-native.sh` and checking that the CI grep sentinel finds no mock strings in `dist/assets/*.js`

---

## Dependencies and Services

### Available Utilities (reused by mock handlers without duplication)

- `DEFAULT_APP_SETTINGS: AppSettingsData` from `src/crosshook-native/src/types/settings.ts` line 69 — the full default settings object; use directly in `store.ts` initial state
- `toSettingsSaveRequest(s: AppSettingsData): SettingsSaveRequest` from `src/crosshook-native/src/types/settings.ts` line 45 — strips computed-only fields before save
- `createDefaultProfile(): GameProfile` from `src/crosshook-native/src/types/profile.ts` line 282 — returns a normalized empty profile; use in handlers and as fixture seed
- `normalizeSerializedGameProfile(profile: SerializedGameProfile): GameProfile` from `src/crosshook-native/src/types/profile.ts` line 218 — used to normalize profiles loaded from fixtures
- `EMPTY_RECENT_FILES: RecentFilesData` — defined locally in `PreferencesContext.tsx` lines 30–34 as a module-scope constant; copy the same shape into `store.ts` (it is not exported from `types/`)

### Required Dependencies

- **Zero new npm packages** — all external libraries evaluated in `research-external.md` were rejected. The adapter uses only `@tauri-apps/api` (already a listed dependency at `^2.0.0`) and `@tauri-apps/plugin-*` (already present). No new `devDependencies` are added.

---

## Gotchas and Warnings

- **`convertFileSrc` synchrony**: `useGameCoverArt.ts` calls `convertFileSrc(customCoverArtPath.trim())` inside a `useMemo` with no `await` — it is called synchronously before the component tree mounts. `MediaSection.tsx` calls it synchronously in a click callback at line 88. The stub in `lib/plugin-stubs/convertFileSrc.ts` must be a plain synchronous function `(path: string) => path`, not a dynamic import. The `main.tsx` eager import ensures it is in the module graph before any component renders.

- **`lib/ipc.ts` must NOT use `import { invoke } from '@tauri-apps/api/core'` at the top-level** — only inside the `if (isTauri())` branch via a dynamic `await import('@tauri-apps/api/core')`. This is required so Vite's tree-shaking can eliminate the Tauri API path in browser builds. The same principle applies to `lib/events.ts` and the `listen` import.

- **`--mode webdev` flag is mandatory**: Plain `npm run dev` (which invokes `vite` without `--mode`) sets `mode = 'development'`. `__WEB_DEV_MODE__` is defined as `mode === 'webdev'`, so it evaluates to `false` and `ensureMocks()` will throw immediately. The `"dev:browser"` npm script hard-codes `--mode webdev` to prevent this; the `ensureMocks()` runtime assertion (`if (!__WEB_DEV_MODE__) throw`) is the second line of defense. Documented as security finding W-1 in `research-security.md`.

- **`App.tsx` component tree ordering**: The `<DevModeBanner />` (Layer 2 chip) must render at the `App()` function level, **outside** `<ProfileProvider>` and `<ProfileHealthProvider>`, so it appears even if context initialization fails. The current `App()` return wraps `<ProfileProvider>` around `<ProfileHealthProvider>` around `<AppShell>`. The chip renders as a direct child of the `<main>` element but a sibling of `<ProfileProvider>`, not inside it.

- **HMR resets `MockStore` singleton**: `getStore()` returns a module-scope object. When Vite HMR reloads `store.ts` or any handler file, the singleton is re-initialized. This is intentional (clean state on hot reload), but contributors working on multi-step mutation flows (e.g., save-then-list) must be aware. Document in `lib/mocks/README.md`.

- **`types/library.ts` is not in the `types/index.ts` barrel**: The current `types/index.ts` exports 22 type modules but **omits `library.ts`**. This means `LibraryCardData` and `LibraryViewMode` are only accessible via direct relative imports. After `ProfileSummary` is added to `library.ts`, the `export * from './library'` line must also be added to `types/index.ts` for mock handlers to import from the barrel (`'../../../types'`).

- **`dev-indicator.css` must not enter production**: The import in `App.tsx` must be inside a `__WEB_DEV_MODE__` conditional. Since CSS cannot be dynamically imported like JS, the pattern is:

  ```ts
  if (__WEB_DEV_MODE__) {
    await import('./lib/dev-indicator.css');
  }
  ```

  Or, place the import inside `DevModeBanner.tsx` itself (CSS imported by the component file is also tree-shaken when the component is never rendered). The safest approach is to import it inside the component file.

- **WebKitGTK ≠ Chrome**: Browser dev mode runs in the system browser. Features that depend on WebKitGTK rendering quirks (scroll physics, font rendering, `color-mix()` support, focus styles) will look different. The `./scripts/dev-native.sh` (full Tauri) run is always required before merging any UI change — browser dev is a design/iteration tool, not the final rendering target.

- **Scroll registry**: Any new `overflow-y: auto` container introduced in the mock UI or the dev-mode chip must be added to the `SCROLLABLE` selector string in `src/crosshook-native/src/hooks/useScrollEnhance.ts` line 8. The current selector string is: `.crosshook-route-card-scroll, .crosshook-page-scroll-body, .crosshook-subtab-content__inner--scroll, .crosshook-console-drawer__body, .crosshook-modal__body, .crosshook-prefix-deps__log-output, .crosshook-discovery-results`.

- **Fixture content policy**: Synthetic game names only; Steam App IDs ≥ 9999001 (non-colliding range); no real file system paths (use `/dev/null/mock/...` or similar); no real personal data. Enforced by PR review in Phase 1 and CI grep in Phase 3.

- **`vite.config.ts` currently uses a flat object export** (`export default defineConfig({ ... })`), not the function form. Adding mode-conditional logic requires changing to `export default defineConfig(({ mode }) => ({ ... }))`. This is a required structural change to the config file.

- **`strictPort: true` already set** in `vite.config.ts` line 12 — for the standard dev path. The webdev-mode override needs to keep `strictPort: true` but also set `host: '127.0.0.1'` to enforce loopback-only binding. The current `host: host || false` uses the `TAURI_DEV_HOST` env var for the Tauri path; the mode-conditional must not break that existing behavior.

---

## Task-Specific Guidance

- **For adapter tasks** (`lib/runtime.ts`, `lib/ipc.ts`, `lib/events.ts`): `lib/runtime.ts` must be zero-dep and have no DOM access — `globalThis.isTauri` is set by the Tauri v2 WebView bridge, not by any npm package. Verify the probe with `console.log(isTauri())` in browser (should be `false`) and in Tauri dev (should be `true`). After implementation, run `rg 'from ["\x27]@tauri-apps/api/core["\x27]' src/crosshook-native/src/ --exclude='lib/ipc.ts' --exclude='lib/plugin-stubs/convertFileSrc.ts'` — must return 0 files.

- **For migration tasks**: Use editor find/replace or `sed` — not codemods. The migration is purely textual: import path + function name. Post-migration verification: `rg "from ['\"]@tauri-apps/api/core['\"]" src/crosshook-native/src/` must return only `lib/ipc.ts` and `lib/plugin-stubs/convertFileSrc.ts` (the Tauri-side of the adapter). `rg "from ['\"]@tauri-apps/api/event['\"]" src/crosshook-native/src/` must return only `lib/events.ts`.

- **For indicator tasks** (`DevModeBanner.tsx`, `dev-indicator.css`): Import `dev-indicator.css` from inside the component so it is automatically excluded from the production bundle when `__WEB_DEV_MODE__ = false` causes the component to never render. The chip's `role="status"` and `aria-label="Browser dev mode active"` attributes are required per the UX spec — no dismiss button.

- **For CI tasks** (`.github/workflows/release.yml`): The `verify:no-mocks` step must run **after** the "Build native AppImage" step (line 100–103) and **before** "Upload AppImage to GitHub Release" (line 133). The grep pattern is `'\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE'` targeting `src/crosshook-native/dist/assets/*.js`. The step must `exit 0` when no files match (using `2>/dev/null` to suppress the "no files found" error from grep) and `exit 1` when any file matches.

- **For documentation tasks** (`AGENTS.md`, `lib/mocks/README.md`): The AGENTS.md Commands block currently has 6 entries (lines 54–61). Add `./scripts/dev-native.sh --browser` as the first entry with a comment `# browser-only, no Rust toolchain required` and the loopback URL note. The `lib/mocks/README.md` is the contributor entry point — it must explain the `register*(map)` function pattern, the `getStore()` singleton and HMR reset behavior, the fixture content policy (no real paths/IDs), and how to trigger the mock layer via `?fixture=` URL params (Phase 3 scope, but the README should pre-document the hook).

- **For mock handler tasks** (Phase 1 boot handlers): The `settings_load` handler must return `getStore().settings` which is initialized from `DEFAULT_APP_SETTINGS`. The `default_steam_client_install_path` handler should return `'/home/devuser/.steam/steam'` (a safe synthetic path — not a real user path). The `profile_list_summaries` handler must return a `ProfileSummary[]` using the type imported from `types/library.ts` after Phase 1 type promotion.
