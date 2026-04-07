# Engineering Practices Research — dev-web-frontend

Codebase evaluation for enabling pure Vite browser development mode in CrossHook's React/TS frontend. The goal is UI/UX iteration without a running Tauri backend, using mock data when `invoke()` is unavailable.

---

## Executive Summary

CrossHook's frontend calls `invoke()` from `@tauri-apps/api/core` at **84 unique call sites** across 35 files, with zero existing mock/fixture infrastructure and zero existing env-mode branching (`import.meta.env` is used only once, for a Steam Deck heuristic in `useGamepadNav.ts`). All Tauri IPC imports are direct — there is no wrapper layer today. The correct implementation is a **thin IPC adapter** (`lib/ipc.ts`) that routes to either the real `invoke` or a mock function map, selected once at startup by a `isTauri()` probe. No MSW, no fixture DSL, no fixture-loading UI — just static TS objects and one function swap point.

---

## Existing Reusable Code

| Location                                                 | What it is                                                                                                                   | Reuse plan                                                                                                                           |
| -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `src/crosshook-native/src/types/index.ts`                | Re-exports all domain types via barrel                                                                                       | All mock fixture return types come from here — no new types needed for most commands                                                 |
| `src/crosshook-native/src/types/profile.ts`              | `GameProfile`, `createDefaultProfile()`, `normalizeSerializedGameProfile()`                                                  | `createDefaultProfile()` is a ready-made fixture for `profile_load` / `profile_list_summaries`                                       |
| `src/crosshook-native/src/types/settings.ts`             | `DEFAULT_APP_SETTINGS`, `AppSettingsData`, `RecentFilesData`                                                                 | `DEFAULT_APP_SETTINGS` is a ready-made fixture for `settings_load`                                                                   |
| `src/crosshook-native/src/types/profile.ts`              | `DEFAULT_GAMESCOPE_CONFIG`, `DEFAULT_MANGOHUD_CONFIG`                                                                        | Ready-made sub-fixtures for profile sections                                                                                         |
| `src/crosshook-native/src/constants/offline.ts`          | `MIN_OFFLINE_READINESS_SCORE`                                                                                                | Reuse in offline readiness fixture                                                                                                   |
| `src/crosshook-native/src/hooks/useGamepadNav.ts:180`    | Only existing `import.meta.env` probe — checks `VITE_STEAM_DECK`                                                             | Pattern to follow for `VITE_MOCK_MODE` env var, but the adapter should use `isTauri()` instead of an env var to avoid new convention |
| `src/crosshook-native/src/utils/optimization-catalog.ts` | `fetchOptimizationCatalog()` — caches `invoke<OptimizationCatalogPayload>('get_optimization_catalog')` in a module-level ref | Already a thin wrapper function; shows that non-React utility functions invoke IPC directly — must be patched to use the adapter     |
| `src/crosshook-native/src/vite-env.d.ts`                 | `ImportMetaEnv` interface with `DEV`, `PROD`, `MODE`                                                                         | Extend here to declare any new `VITE_` vars rather than creating a new `.d.ts`                                                       |
| `src/crosshook-native/vite.config.ts`                    | `envPrefix: ['VITE_', 'TAURI_ENV_*']`                                                                                        | Confirms `VITE_` env vars are already forwarded to the browser bundle                                                                |

**What does NOT exist today:**

- No `lib/` directory anywhere under `src/crosshook-native/src/`
- No mock files, fixture files, or stub objects anywhere in the repo
- No IPC wrapper hook (`useTauriCommand`, `useInvoke`, etc.)
- No dev banner or mock-mode indicator component
- No `import.meta.env.DEV` branches in component or hook code

---

## Modularity Design

Four modules. No more.

### 1. `src/crosshook-native/src/lib/runtime.ts`

**Purpose**: single function that answers "is the app running inside Tauri?"

```ts
export function isTauri(): boolean {
  return typeof (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !== 'undefined';
}
```

**Why a standalone module**: every other module imports this. It must not import anything from the project to avoid circular deps. It is also the only piece that needs to change if Tauri ever changes its detection signal — one place, not 35.

**Testability**: pure function, no React, no Vite, runs in Node with `jsdom` or without any DOM at all.

### 2. `src/crosshook-native/src/lib/ipc.ts`

**Purpose**: single `callCommand` function that the entire app uses instead of calling `invoke()` directly.

```ts
import { isTauri } from './runtime';
import { getMockRegistry } from './mocks/index';

export async function callCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(command, args);
  }
  const registry = getMockRegistry();
  const handler = registry[command];
  if (!handler) {
    throw new Error(`[dev-mock] No mock registered for command: ${command}`);
  }
  return handler(args) as Promise<T>;
}
```

Key decisions:

- Dynamic import of `@tauri-apps/api/core` under the Tauri branch so the Tauri module is not bundled into the pure-browser build. Vite's tree-shaking handles this.
- Throws on missing mock rather than returning `undefined` — fail-fast surfaces gaps immediately during iteration.
- No generic `Commands` map type at the `callCommand` call site — the generics live in each hook's own `invoke<T>` annotation, which moves unchanged to `callCommand<T>`. This is the lowest-friction migration path.

**Typed `Commands` map (optional, additive)**: A typed registry can be layered on later without changing call sites:

```ts
// lib/commands.ts — additive, not blocking
type Commands = {
  settings_load: { args: void; return: AppSettingsData };
  profile_list: { args: void; return: string[] };
  profile_load: { args: { name: string }; return: SerializedGameProfile };
  // ... one per command
};
export function callCommand<K extends keyof Commands>(
  name: K,
  args?: Commands[K]['args']
): Promise<Commands[K]['return']>;
```

All return types already exist in `src/crosshook-native/src/types/` — no new type invention needed. This is a nice-to-have for autocomplete; the untyped string version is sufficient for the initial browser-dev goal.

**Testability**: can be tested by overriding the mock registry in test setup. No Vite needed.

### 3. `src/crosshook-native/src/lib/mocks/index.ts`

**Purpose**: exports a plain `Record<string, MockHandler>` containing all command mocks.

```ts
type MockHandler = (args?: Record<string, unknown>) => Promise<unknown>;
export type MockRegistry = Record<string, MockHandler>;

export function getMockRegistry(): MockRegistry {
  return {
    settings_load: async () => DEFAULT_APP_SETTINGS,
    recent_files_load: async () => ({ game_paths: [], trainer_paths: [], dll_paths: [] }),
    settings_save: async () => undefined,
    recent_files_save: async () => undefined,
    profile_list: async () => ['Elden Ring', 'Cyberpunk 2077'],
    profile_list_summaries: async () => MOCK_LIBRARY_SUMMARIES,
    profile_list_favorites: async () => [],
    profile_load: async (args) => MOCK_PROFILES[(args as { name: string }).name] ?? createDefaultProfile(),
    default_steam_client_install_path: async () => '/home/user/.steam/steam',
    // ... remaining commands
  };
}
```

**Why not a class or factory**: there are two places in the codebase (e.g. `useProfile.ts` and `PreferencesContext.tsx`) that both call `settings_load`. A plain object map handles N callers with zero plumbing. No factory is needed until the same _per-command fixture variant_ pattern appears three times (Rule of Three).

**Why not MSW**: MSW intercepts HTTP fetch/XHR. Tauri `invoke()` is a custom IPC mechanism over a WebView bridge — MSW cannot intercept it. The function-map approach directly replaces the call path at the right layer.

**Why not `@tauri-apps/api/mocks`**: `mockIPC` works in a plain browser — its `mockInternals()` function creates `window.__TAURI_INTERNALS__` via `??=` if it does not exist (verified: `tauri-apps/tauri:packages/api/src/mocks.ts`, function `mockInternals()`). The reason to prefer the hand-rolled adapter is coverage: `mockIPC` handles `invoke` only. It does not cover `listen()` event subscriptions, `@tauri-apps/plugin-dialog`, `plugin-shell`, `plugin-fs`, or `convertFileSrc`. The hand-rolled adapter covers all of these at one boundary. Reject for scope, not for browser incompatibility.

### 4. Per-command fixture files

**Purpose**: keep mock data close to the relevant type.

Suggested layout (only create as needed, not all upfront):

```
lib/mocks/
  index.ts              ← registry
  profiles.ts           ← MOCK_PROFILES, MOCK_LIBRARY_SUMMARIES
  settings.ts           ← already exists as DEFAULT_APP_SETTINGS in types/settings.ts, just re-export
  health.ts             ← MOCK_HEALTH_SUMMARY if health dashboard iteration is needed
```

Do not create a file per command — group by domain. Do not create all files upfront; create them as UI areas are iterated.

### 5. Dev-mode banner component

**Purpose**: visible indicator in browser mode so reviewers and developers know they are looking at mock data.

```tsx
// src/crosshook-native/src/components/layout/DevModeBanner.tsx
import { isTauri } from '../../lib/runtime';

export function DevModeBanner() {
  if (isTauri()) return null;
  return (
    <div className="crosshook-dev-banner" role="status" aria-live="polite">
      Browser dev mode — mock data active
    </div>
  );
}
```

Render once in `App.tsx` above `<ProfileProvider>`. The `isTauri()` check makes it a no-op in production with no bundle cost from conditional rendering.

---

## KISS Assessment

| Question                                         | Answer                                                                                                                                                                                                                                                                                                  |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Need a fixture-toggle DSL?                       | No. Static objects are sufficient. Adding a DSL adds indirection with no payoff for UI iteration.                                                                                                                                                                                                       |
| Need MSW?                                        | No. MSW cannot intercept Tauri IPC. Wrong layer.                                                                                                                                                                                                                                                        |
| Need a fixture-loading UI?                       | No. Edit `lib/mocks/profiles.ts`, Vite HMR picks it up in ~200ms. Zero tooling needed.                                                                                                                                                                                                                  |
| Need `@tauri-apps/api/mocks`?                    | No — not because it fails in browsers (it works; `mockInternals()` creates `window.__TAURI_INTERNALS__` via `??=`), but because it only covers `invoke`. CrossHook also needs `listen()`, three plugin packages, and `convertFileSrc` mocked. The hand-rolled adapter covers all of these in one place. |
| Need a typed `Commands` map at launch?           | No — nice to have, add as an ergonomic layer after the adapter works. Does not gate browser-dev mode.                                                                                                                                                                                                   |
| Need dynamic fixture loading (JSON files, etc.)? | No. The user's stated need is "strictly for UI/UX iteration". Static TS is hot-reloadable and type-checked.                                                                                                                                                                                             |

**Minimum viable implementation**: `lib/runtime.ts` (6 lines) + `lib/ipc.ts` (15 lines) + `lib/mocks/index.ts` (30–50 lines for core commands) + migrate all 84 `invoke(` call sites to `callCommand(`. The banner is optional but strongly recommended for clarity.

---

## Abstraction vs. Repetition (Rule of Three)

Current state: **zero** mock handlers exist. The first time a mock is written, write it inline in `index.ts`. The second time a _variant_ of the same fixture is needed (e.g., a profile with no Steam ID), add a named constant. Only if a _third_ distinct fixture variant for the same command is needed should a helper or factory emerge.

Concretely: `createDefaultProfile()` in `types/profile.ts` is already the one profile fixture. Reuse it directly. Do not extract a `MockProfileFactory` until two additional profile variants with meaningfully different shapes are needed for iteration.

---

## Interface Design

The untyped adapter is sufficient for browser-dev mode. The typed `Commands` map below is an additive improvement that can be built on top without changing any call site.

**All return types already exist** in `src/crosshook-native/src/types/`:

| Command                    | Return type                  | Source file                                      |
| -------------------------- | ---------------------------- | ------------------------------------------------ |
| `settings_load`            | `AppSettingsData`            | `types/settings.ts`                              |
| `recent_files_load`        | `RecentFilesData`            | `types/settings.ts`                              |
| `profile_list`             | `string[]`                   | built-in                                         |
| `profile_list_summaries`   | `ProfileSummary[]`           | `hooks/useLibrarySummaries.ts` (local interface) |
| `profile_load`             | `SerializedGameProfile`      | `types/profile.ts`                               |
| `profile_list_favorites`   | `string[]`                   | built-in                                         |
| `launch_game`              | `LaunchResult`               | `types/launch.ts`                                |
| `check_readiness`          | `ReadinessCheckResult`       | `types/onboarding.ts`                            |
| `batch_validate_profiles`  | `EnrichedHealthSummary`      | `types/health.ts`                                |
| `get_optimization_catalog` | `OptimizationCatalogPayload` | `utils/optimization-catalog.ts`                  |
| `list_proton_installs`     | `ProtonInstallOption[]`      | `types/proton.ts`                                |

One gap: `ProfileSummary` in `hooks/useLibrarySummaries.ts` is a local (non-exported) interface. It should be moved to `types/library.ts` so the mock file can import it cleanly.

---

## Testability Patterns

The proposed structure supports unit tests without Vite:

- `lib/runtime.ts`: pure function — assert `isTauri()` returns `false` in JSDOM with no `__TAURI_INTERNALS__` on `window`.
- `lib/ipc.ts`: inject a test registry by temporarily replacing what `getMockRegistry()` returns, or by passing the registry as an optional parameter to `callCommand`. No React, no Vite.
- `lib/mocks/index.ts`: each handler is `async () => staticValue` — trivially assertable.
- Individual hook tests: replace `callCommand` with a jest/vitest mock at the module boundary, same as any async function boundary.

Note: CrossHook currently has **no configured frontend test framework** (confirmed from `package.json` — no vitest, no jest, no testing-library). Unit testability is achievable but requires adding a test runner first, which is out of scope for the browser-dev feature itself.

---

## Build vs. Depend

| Option                                          | Cost                                                                       | Benefit                                                                                                            | Verdict                                                                                                                                                                                                                                                                                                    |
| ----------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Hand-rolled `callCommand` adapter               | ~50 lines of new code, migration of ~84 call sites                         | Zero new deps, explicit at every callsite, loud on missing mocks, no production import risk                        | **Recommended**                                                                                                                                                                                                                                                                                            |
| `@tauri-apps/api/mocks` (`mockIPC`)             | Zero new deps (already in `@tauri-apps/api ^2.0.0`), zero callsite changes | `mockIPC` creates `window.__TAURI_INTERNALS__` itself — works in pure browser. Also provides `mockConvertFileSrc`. | **Viable alternative** — but defaults to silent `undefined` on unregistered commands; requires explicit throw in callback. Production import guard discipline required.                                                                                                                                    |
| MSW                                             | New `devDependency`, service worker setup                                  | Industry standard for REST APIs                                                                                    | **Wrong layer** — MSW intercepts `fetch`/XHR; Tauri IPC is a WebView bridge call. No frontend `fetch()` calls exist in this codebase.                                                                                                                                                                      |
| Vite `resolve.alias` for `@tauri-apps/api/core` | Alias config change only, zero callsite changes                            | Clean swap for plugin modules                                                                                      | **Partial fit** — works cleanly for `@tauri-apps/api/event`, `plugin-dialog`, `plugin-shell`. For `@tauri-apps/api/core`, the alias module still requires the same registry dispatch internally, just hidden from callsites. Preferred for the three plugin modules; `callCommand` preferred for `invoke`. |

The `callCommand` adapter and `mockIPC` are both zero-new-dependency options. The meaningful trade-off: `callCommand` requires ~84 mechanical callsite changes but is explicit and loud on missing mocks; `mockIPC` requires zero callsite changes but is silent on missing mocks unless you add explicit throws in the handler callback — at which point the internal complexity is identical.

---

## Call Site Migration Scope

84 `invoke()` call sites across 35 files:

| Category                   | Files                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Approximate call count |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------- |
| Hooks                      | `useProfile.ts`, `useLaunchState.ts`, `useCommunityProfiles.ts`, `useInstallGame.ts`, `useUpdateGame.ts`, `useRunExecutable.ts`, `useOfflineReadiness.ts`, `useProfileHealth.ts`, `useGameCoverArt.ts`, `useGameMetadata.ts`, `usePrefixDeps.ts`, `usePrefixStorageManagement.ts`, `useProtonUp.ts`, `useProtonMigration.ts`, `useProtonInstalls.ts`, `useLauncherManagement.ts`, `useProtonDbLookup.ts`, `useProtonDbSuggestions.ts`, `useMangoHudPresets.ts`, `useTrainerTypeCatalog.ts`, `useTrainerDiscovery.ts`, `useExternalTrainerSearch.ts`, `usePreviewState.ts`, `useOnboarding.ts`, `useSetTrainerVersion.ts`, `useLibrarySummaries.ts`, `useGameDetailsProfile.ts` | ~60                    |
| Context providers          | `PreferencesContext.tsx`, `ProfileContext.tsx` (via `useProfile`)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              | ~10                    |
| Components (direct invoke) | `LaunchPanel.tsx`, `LaunchPage.tsx`, `HealthDashboardPage.tsx`, `ProfilesPage.tsx`, `ProfileActions.tsx`, `TrainerDiscoveryPanel.tsx`, `OnboardingWizard.tsx`, `AutoPopulate.tsx`, `CommunityImportWizardModal.tsx`, `LauncherExport.tsx`, `SettingsPanel.tsx`, `SteamLaunchOptionsPanel.tsx`, `MediaSection.tsx`                                                                                                                                                                                                                                                                                                                                                              | ~14                    |
| Non-React utilities        | `utils/optimization-catalog.ts`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | 1                      |

The 13 components that call `invoke()` directly (not through a hook) represent a mild architectural smell — business logic leaking into component layer — but fixing that is outside the scope of browser-dev mode. For now, `callCommand` replaces `invoke` one-for-one.

The `listen()` import from `@tauri-apps/api/event` appears in 10 files and requires a separate stub. `listen` returns an `UnlistenFn` promise; the mock can return `Promise.resolve(() => undefined)`.

---

## Open Questions

1. **`convertFileSrc` stub**: `useGameCoverArt.ts` and `MediaSection.tsx` use `convertFileSrc` from `@tauri-apps/api/core`. In browser mode this should be a passthrough (`(path: string) => path`). If using `mockIPC`, `@tauri-apps/api/mocks` exports `mockConvertFileSrc` which handles this directly. If using `callCommand`, re-export `convertFileSrc` from `lib/ipc.ts` with a browser-mode passthrough branch.

2. **`listen()` events**: the mock `listen` stub returns a no-op unlisten function. This means event-driven state updates (auto-load profile, launch progress, console output) will not fire in browser mode. This is acceptable for static UI iteration but should be documented so developers don't wonder why launch phase UI never progresses.

3. **Plugin stubs** (`@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-shell`, `@tauri-apps/plugin-fs`): `utils/dialog.ts` imports `open`/`save` from `plugin-dialog`; `ExternalResultsSection.tsx` uses `plugin-shell`; `CommunityBrowser.tsx` uses `plugin-dialog`. These are separate from `invoke()` and need their own stubs — likely a single `lib/mocks/tauri-plugins.ts` that exports no-op versions.

4. **`ProfileSummary` type visibility**: as noted above, the interface is local to `useLibrarySummaries.ts` and needs to be exported from `types/library.ts` before a typed mock can be written cleanly.

5. **Script integration**: `dev-native.sh` currently has no `--dev` flag. A `--browser` flag that runs `npm run dev` (plain Vite, no `tauri dev`) would be the clean entry point. This is a 3-line shell addition.
