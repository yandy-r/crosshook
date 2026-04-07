# Technical Architecture — dev-web-frontend

Technical design for enabling pure Vite browser development mode in CrossHook. Goal: `./scripts/dev-native.sh --dev` starts a `localhost:5173` dev server, the React app boots in Chrome/Firefox, and all Tauri IPC calls route to local mock handlers rather than a native backend. Strictly a UI/UX iteration convenience — no production impact.

---

## Executive Summary

CrossHook's React frontend calls `invoke()` from `@tauri-apps/api/core` at 84 call sites across 42 files. There is currently no IPC abstraction layer, no mock infrastructure, and no `lib/` directory. The architecture introduces a thin adapter layer (`lib/ipc.ts`, `lib/events.ts`, `lib/runtime.ts`) that is the single decision point for "are we in Tauri or browser?" and routes accordingly. All mock code is gated behind a dynamic import that Vite's tree-shaker eliminates from the production bundle. The entire change is backwards-compatible: in Tauri mode, every call passes through to the real `invoke` with identical semantics.

The mock layer is activated by `!isTauri()` — a runtime probe, not a build-time env var. This means contributors who open `localhost:5173` directly (via `npm run dev:web`) get mock mode automatically. The `window.__TAURI_INTERNALS__` object injected by the Tauri WebView bridge is the official Tauri v2 detection API; it is absent in Chrome and Firefox.

---

## Relevant Files

**New files (to be created):**

- `src/crosshook-native/src/lib/runtime.ts` — `isTauri()` probe, single source of truth for runtime detection
- `src/crosshook-native/src/lib/ipc.ts` — `callCommand<T>` adapter replacing all direct `invoke()` calls
- `src/crosshook-native/src/lib/events.ts` — `subscribeEvent<T>` adapter replacing all direct `listen()` calls, plus in-process event bus for mock event emission
- `src/crosshook-native/src/lib/plugin-stubs/dialog.ts` — re-exports real `@tauri-apps/plugin-dialog` in Tauri mode; returns null no-op in browser mode
- `src/crosshook-native/src/lib/plugin-stubs/shell.ts` — same pattern for `@tauri-apps/plugin-shell`
- `src/crosshook-native/src/lib/plugin-stubs/event.ts` — Vite alias target for `@tauri-apps/api/event` (delegates to `lib/events.ts`)
- `src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts` — `(path: string) => path` passthrough in browser mode
- `src/crosshook-native/src/lib/mocks/index.ts` — `registerMocks()` + fixture-state resolver (module-scope init, before React mount)
- `src/crosshook-native/src/lib/mocks/store.ts` — in-memory mutable state for round-trip commands (profile save → profile list)
- `src/crosshook-native/src/lib/mocks/README.md` — how to add a handler (one worked example)
- `src/crosshook-native/src/lib/mocks/handlers/settings.ts` — boot-critical: `settings_load`, `recent_files_load`, `settings_save`, `recent_files_save`, `default_steam_client_install_path`, `settings_save_steamgriddb_key`
- `src/crosshook-native/src/lib/mocks/handlers/profile.ts` — boot-critical: `profile_list`, `profile_load`, `profile_list_summaries`, `profile_list_favorites`, `profile_save`, `profile_duplicate`, `profile_rename`, `profile_delete`, `profile_set_favorite`
- `src/crosshook-native/src/components/DevModeChip.tsx` — `position: fixed` badge (not sidebar-bound)
- `src/crosshook-native/.env.web-dev` — sets `VITE_WEB_DEV=true` for the `web-dev` Vite mode

**Modified files (Phase 1):**

- `scripts/dev-native.sh` — add `--dev` branch (3–5 lines)
- `src/crosshook-native/package.json` — add `"dev:web": "vite --mode web-dev"` script
- `src/crosshook-native/vite.config.ts` — add `({ mode })` config factory; inject plugin-stub aliases in `web-dev` mode; bind `server.host = '127.0.0.1'` in `web-dev` mode
- `src/crosshook-native/src/vite-env.d.ts` — add `readonly VITE_WEB_DEV?: string` to `ImportMetaEnv`
- `src/crosshook-native/src/App.tsx` — render `<DevModeChip>` outside router tree; add `?onboarding=show` state injection
- All 42 files containing `invoke(` — mechanical `invoke(` → `callCommand(` migration
- All 13 files containing `listen(` from `@tauri-apps/api/event` — `listen(` → `subscribeEvent(` migration
- 2 files with `@tauri-apps/plugin-dialog` imports (`utils/dialog.ts`, `components/CommunityBrowser.tsx`)
- 4 files with `@tauri-apps/plugin-shell` imports (`SettingsPanel.tsx`, `TrainerDiscoveryPanel.tsx`, `ExternalResultsSection.tsx`, `ProtonDbLookupCard.tsx`)
- 2 files with `convertFileSrc` imports (`hooks/useGameCoverArt.ts`, `components/profile-sections/MediaSection.tsx`)
- `.github/workflows/release.yml` — add CI sentinel grep after AppImage build step

---

## Architecture Design

### Runtime Detection — `lib/runtime.ts`

Single exported function, zero dependencies:

```ts
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
```

The `__TAURI_INTERNALS__` object is injected by the Tauri WebView bridge before any JavaScript executes. It is absent in Chrome/Firefox. This is the official Tauri v2 detection signal. Every other module in the new `lib/` directory imports this function — no other module duplicates the detection logic.

**Why a standalone module**: prevents circular dependencies (everything imports `runtime.ts`, it imports nothing from the project); one change point if Tauri ever changes the detection signal.

### IPC Adapter — `lib/ipc.ts`

```ts
import type { InvokeArgs } from '@tauri-apps/api/core';
import { isTauri } from './runtime';

type Handler = (args: unknown) => unknown | Promise<unknown>;
// Promise latch prevents concurrent registerMocks() calls on parallel first invocations
let mocksPromise: Promise<Map<string, Handler>> | null = null;

async function ensureMocks(): Promise<Map<string, Handler>> {
  if (!mocksPromise) {
    mocksPromise = import('./mocks/index').then((m) => m.registerMocks());
  }
  return mocksPromise;
}

export async function callCommand<T>(name: string, args?: InvokeArgs): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(name, args);
  }
  const map = await ensureMocks();
  const handler = map.get(name);
  if (!handler) {
    throw new Error(
      `[mock] Unhandled command: "${name}". Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md`
    );
  }
  if (import.meta.env.DEV) {
    console.debug('[mock] callCommand', name, args);
  }
  return handler(args ?? {}) as Promise<T>;
}
```

Two critical properties:

1. **Both `import('@tauri-apps/api/core')` AND `import('./mocks/index')` are dynamic.** Vite sees two dynamic imports behind mutually exclusive runtime branches. In the production AppImage, `isTauri()` is always `true`, so the mock import is dead code — Vite eliminates it from the bundle.

2. **Promise latch in `ensureMocks`.** The `import('./mocks/index')` runs once on first `callCommand` invocation. `PreferencesContext` fires 3 parallel `callCommand` calls on mount — without the latch, all three would initiate concurrent `import('./mocks/index')` calls. With the latch, the first call creates the shared promise and subsequent calls reuse it.

### Event Adapter — `lib/events.ts`

```ts
import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { isTauri } from './runtime';

type Listener = (payload: unknown) => void;
const browserBus = new Map<string, Set<Listener>>();

export async function subscribeEvent<T>(name: string, handler: EventCallback<T>): Promise<UnlistenFn> {
  if (isTauri()) {
    const { listen } = await import('@tauri-apps/api/event');
    return listen<T>(name, handler);
  }
  const wrapped: Listener = (payload) => handler({ event: name, id: 0, payload: payload as T });
  if (!browserBus.has(name)) browserBus.set(name, new Set());
  browserBus.get(name)!.add(wrapped);
  return () => {
    browserBus.get(name)?.delete(wrapped);
  };
}

export function emitMockEvent(name: string, payload: unknown): void {
  if (isTauri()) return;
  browserBus.get(name)?.forEach((fn) => fn(payload));
}
```

The return type (`Promise<() => void>`) exactly matches `@tauri-apps/api/event`'s `listen()` contract, which all existing hooks handle via `.then(f => f())` cleanup in `useEffect` teardowns. `emitMockEvent` is called by mock handlers (e.g. `handlers/launch.ts`) to drive event-dependent state transitions.

### Fixture State Resolver — `lib/mocks/index.ts`

The fixture state resolver **must execute at module-scope** — as `const` initializations before any function body — so that it runs before React mounts. `PreferencesContext` and `ProfileContext` call `callCommand()` in their mount effects; by the time these fire, `registerMocks()` must have already applied the correct fixture variant.

```ts
// lib/mocks/index.ts — module-scope (runs before registerMocks is called)
const params = new URLSearchParams(typeof window !== 'undefined' ? window.location.search : '');

export const FIXTURE =
  (['populated', 'empty', 'error', 'loading'] as const).find((s) => s === params.get('fixture')) ?? 'populated';

export const ERRORS_ENABLED = params.get('errors') === 'true';

export const DELAY_MS = Math.max(0, parseInt(params.get('delay') ?? '0', 10) || 0);

export const SHOW_ONBOARDING = params.get('onboarding') === 'show';

type Handler = (args: unknown) => unknown | Promise<unknown>;

/** Commands that always succeed even when ERRORS_ENABLED is true */
const READ_COMMANDS = new Set([
  'settings_load',
  'recent_files_load',
  'default_steam_client_install_path',
  'profile_list',
  'profile_list_favorites',
  'profile_list_summaries',
  'profile_load',
  'get_cached_health_snapshots',
  'batch_validate_profiles',
  'get_cached_offline_readiness_snapshots',
  'check_readiness',
  'get_optimization_catalog',
  'get_trainer_type_catalog',
  'get_mangohud_presets',
]);

function wrapHandler(name: string, fn: Handler): Handler {
  return async (args) => {
    if (DELAY_MS > 0) await new Promise((r) => setTimeout(r, DELAY_MS));
    if (ERRORS_ENABLED && !READ_COMMANDS.has(name)) {
      throw new Error(`[mock] Simulated error for command: ${name}`);
    }
    return fn(args);
  };
}

export function registerMocks(): Map<string, Handler> {
  const map = new Map<string, Handler>();
  registerSettings(map);
  registerProfiles(map);
  // ... other domains
  for (const [name, fn] of map) {
    map.set(name, wrapHandler(name, fn));
  }
  return map;
}
```

### Vite Mode Strategy

A named Vite mode `web-dev` activates the mock layer configuration. `src/crosshook-native/.env.web-dev` sets `VITE_WEB_DEV=true`. The `vite.config.ts` mode factory injects stub aliases and restricts the dev server to loopback only in this mode:

```ts
// vite.config.ts
import { defineConfig, type UserConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { resolve } from 'path';

const host = process.env.TAURI_DEV_HOST;
const isDebug = !!process.env.TAURI_ENV_DEBUG;

export default defineConfig(({ mode }) => {
  const isWebDev = mode === 'web-dev';
  const srcDir = resolve(__dirname, 'src');

  const config: UserConfig = {
    plugins: [react()],
    clearScreen: false,
    server: {
      port: 5173,
      strictPort: true,
      host: isWebDev ? '127.0.0.1' : host || false,
      hmr: !isWebDev && host ? { protocol: 'ws', host, port: 1421 } : undefined,
      watch: { ignored: ['**/src-tauri/**'] },
    },
    envPrefix: ['VITE_', 'TAURI_ENV_*'],
    build: {
      target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
      minify: isDebug ? false : 'oxc',
      sourcemap: isDebug,
    },
  };

  if (isWebDev) {
    config.resolve = {
      alias: {
        '@tauri-apps/api/event': resolve(srcDir, 'lib/plugin-stubs/event.ts'),
        '@tauri-apps/plugin-dialog': resolve(srcDir, 'lib/plugin-stubs/dialog.ts'),
        '@tauri-apps/plugin-shell': resolve(srcDir, 'lib/plugin-stubs/shell.ts'),
      },
    };
  }

  return config;
});
```

The `@tauri-apps/api/core` package is **not** aliased — `callCommand` handles it via the explicit registry dispatch. Aliasing `api/core` would need to re-implement the registry inside the alias anyway, with no benefit.

### Plugin Stubs — `lib/plugin-stubs/`

Each plugin stub follows the same re-export pattern:

```ts
// lib/plugin-stubs/dialog.ts
import { isTauri } from '../runtime';

export async function open(options?: unknown): Promise<string | null> {
  if (isTauri()) {
    const { open: realOpen } = await import('@tauri-apps/plugin-dialog');
    return realOpen(options as never);
  }
  return null; // Callers treat null as user-cancelled — correct semantic
}

export async function save(options?: unknown): Promise<string | null> {
  if (isTauri()) {
    const { save: realSave } = await import('@tauri-apps/plugin-dialog');
    return realSave(options as never);
  }
  return null;
}
```

`lib/plugin-stubs/convertFileSrc.ts` is a passthrough — fixture path strings render directly as `<img src>` values:

```ts
export function convertFileSrc(path: string, _protocol?: string): string {
  return path;
}
```

The two files that use `convertFileSrc` (`hooks/useGameCoverArt.ts`, `components/profile-sections/MediaSection.tsx`) update their import from `@tauri-apps/api/core` to `../../lib/plugin-stubs/convertFileSrc`. Unlike the three plugin modules, `convertFileSrc` is NOT covered by the Vite alias because it lives inside `@tauri-apps/api/core` alongside `invoke` — aliasing the whole package would break `invoke`.

### DevModeChip Component

The chip must be `position: fixed; bottom: 12px; right: 12px` — it **must not** be placed inside the sidebar. The sidebar applies `display: none` to `.crosshook-sidebar__brand` and `.crosshook-sidebar__status-group` when `data-collapsed="true"` (`sidebar.css` lines 203–237), which would hide a sidebar-embedded chip whenever the sidebar is collapsed.

```tsx
// src/crosshook-native/src/components/DevModeChip.tsx
import { isTauri } from '../lib/runtime';
import { FIXTURE, ERRORS_ENABLED, DELAY_MS } from '../lib/mocks/index';

export function DevModeChip() {
  if (isTauri()) return null;

  const parts = [`DEV · ${FIXTURE}`];
  if (ERRORS_ENABLED) parts.push('errors');
  if (DELAY_MS > 0) parts.push(`+${DELAY_MS}ms`);
  const label = parts.join(' · ').toUpperCase();

  return (
    <div
      role="status"
      aria-label={`Developer mode active. Fixture: ${FIXTURE}`}
      style={{
        position: 'fixed',
        bottom: 12,
        right: 12,
        zIndex: 9999,
        fontSize: '0.7rem',
        letterSpacing: '0.08em',
        padding: '3px 8px',
        borderRadius: 4,
        pointerEvents: 'none',
        userSelect: 'none',
        background: 'var(--crosshook-autosave-warning-bg)',
        border: '1px solid var(--crosshook-autosave-warning-border)',
        color: 'var(--crosshook-color-warning)',
      }}
    >
      {label}
    </div>
  );
}
```

Rendered from `App.tsx` outside the `<ProfileProvider>` and router tree. The `isTauri()` guard makes it a pure no-op in the AppImage.

The chip imports `FIXTURE`, `ERRORS_ENABLED`, and `DELAY_MS` directly from `lib/mocks/index.ts`. These are module-scope `const` values resolved before React mounts — they do not change during a session. No React state needed.

### Onboarding Wizard Opt-in

The `onboarding-check` Tauri event never fires in browser mode. `App.tsx` checks `SHOW_ONBOARDING` from `lib/mocks/index.ts` in the mount effect and injects the wizard state directly:

```tsx
// App.tsx — inside AppShell's useEffect
useEffect(() => {
  if (!isTauri() && SHOW_ONBOARDING) {
    setShowOnboarding(true);
    return;
  }
  const p = subscribeEvent<OnboardingCheckPayload>('onboarding-check', (event) => {
    if (event.payload.show && !event.payload.has_profiles) setShowOnboarding(true);
  });
  return () => {
    p.then((f) => f());
  };
}, []);
```

In production the `isTauri()` guard ensures this block is dead code.

### Script Flag — `scripts/dev-native.sh`

```bash
case "${1:-}" in
  --dev|--browser)
    cd "$NATIVE_DIR"
    [[ -x "$NATIVE_DIR/node_modules/.bin/vite" ]] || npm ci
    echo "Starting CrossHook frontend-only dev server (browser mock IPC)..."
    echo "  -> http://localhost:5173"
    echo "  -> Default fixture: populated"
    echo "  -> Override: ?fixture=empty|error|loading  ?errors=true  ?delay=<ms>  ?onboarding=show"
    exec npm run dev:web
    ;;
  --help|-h)
    usage
    exit 0
    ;;
  "")
    ;;
  *)
    echo "Error: unknown argument: $1" >&2
    usage >&2
    exit 1
    ;;
esac
```

`--dev` is the canonical name. `--browser` is accepted as an alias. `npm run dev:web` maps to `vite --mode web-dev`.

### package.json Changes

```json
"scripts": {
  "dev":     "vite",
  "dev:web": "vite --mode web-dev",
  "build":   "tsc && vite build",
  "preview": "vite preview",
  "tauri":   "tauri"
}
```

### vite-env.d.ts Changes

```ts
interface ImportMetaEnv {
  readonly DEV: boolean;
  readonly PROD: boolean;
  readonly MODE: string;
  readonly BASE_URL: string;
  /** Set to 'true' when running in web-dev (browser-only mock) mode via .env.web-dev */
  readonly VITE_WEB_DEV?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
```

### Mock Data Boot Sequence

`PreferencesContext.tsx` (line 43–47) fires 3 commands in parallel before any UI renders. After migration these become the first `callCommand` calls. The Promise latch in `ensureMocks` ensures all three resolve from the same mock registry instance.

**Minimum working set for app boot (Phase 1):**

| Command                                  | Handler file           | Fixture source                                                      |
| ---------------------------------------- | ---------------------- | ------------------------------------------------------------------- |
| `settings_load`                          | `handlers/settings.ts` | `DEFAULT_APP_SETTINGS` from `types/settings.ts`                     |
| `recent_files_load`                      | `handlers/settings.ts` | `{ game_paths: [], trainer_paths: [], dll_paths: [] }`              |
| `settings_save`                          | `handlers/settings.ts` | `async () => undefined`                                             |
| `recent_files_save`                      | `handlers/settings.ts` | `async () => undefined`                                             |
| `default_steam_client_install_path`      | `handlers/settings.ts` | `async () => '/home/user/.steam/steam'`                             |
| `profile_list`                           | `handlers/profile.ts`  | `async () => ['Elden Ring', 'Cyberpunk 2077']`                      |
| `profile_list_favorites`                 | `handlers/profile.ts`  | `async () => []`                                                    |
| `profile_list_summaries`                 | `handlers/profile.ts`  | Array of `ProfileSummary` from `createDefaultProfile()`             |
| `profile_load`                           | `handlers/profile.ts`  | `async ({ name }) => MOCK_PROFILES[name] ?? createDefaultProfile()` |
| `get_cached_health_snapshots`            | `handlers/health.ts`   | `async () => []`                                                    |
| `batch_validate_profiles`                | `handlers/health.ts`   | `async () => ({ profiles: {} })`                                    |
| `get_cached_offline_readiness_snapshots` | `handlers/health.ts`   | `async () => {}`                                                    |

### Tree-Shaking & Build-Time Elimination

The mock subtree must not appear in the production AppImage. Three enforcement layers:

1. **Dynamic import gating**: `import('./mocks/index')` is behind `!isTauri()`. In the AppImage, `isTauri()` is always `true`. Rollup marks the mock import as unreachable and omits the chunk.

2. **Mode alias exclusion**: Vite plugin-stub aliases are only injected when `mode === 'web-dev'`. Production builds run without `--mode web-dev`, so stub modules are never bundled.

3. **CI sentinel grep**: After the AppImage build step in `.github/workflows/release.yml`:

   ```yaml
   - name: Verify mock code not bundled
     run: |
       if grep -r "registerMocks\|\[mock\] callCommand" src/crosshook-native/dist/ 2>/dev/null; then
         echo "ERROR: Mock code found in production bundle"
         exit 1
       fi
   ```

The dynamic import must stay dynamic — never convert `import('./mocks/index')` to a top-level static import.

---

## Call Site Migration

### `invoke` → `callCommand` (84 sites across 42 files)

Mechanical find-and-replace:

- Import: `import { invoke } from '@tauri-apps/api/core'` → `import { callCommand } from '../lib/ipc'`
- Call: `invoke<T>('command_name', args)` → `callCommand<T>('command_name', args)`

Files requiring migration:

| Category                      | Files                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Approx calls |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------ |
| Hooks (27 files)              | `useProfile.ts`, `useLaunchState.ts`, `useCommunityProfiles.ts`, `useInstallGame.ts`, `useUpdateGame.ts`, `useRunExecutable.ts`, `useOfflineReadiness.ts`, `useProfileHealth.ts`, `useGameCoverArt.ts`, `useGameMetadata.ts`, `usePrefixDeps.ts`, `usePrefixStorageManagement.ts`, `useProtonUp.ts`, `useProtonMigration.ts`, `useProtonInstalls.ts`, `useLauncherManagement.ts`, `useProtonDbLookup.ts`, `useProtonDbSuggestions.ts`, `useMangoHudPresets.ts`, `useTrainerTypeCatalog.ts`, `useTrainerDiscovery.ts`, `useExternalTrainerSearch.ts`, `usePreviewState.ts`, `useOnboarding.ts`, `useSetTrainerVersion.ts`, `useLibrarySummaries.ts`, `useGameDetailsProfile.ts` | ~60          |
| Context providers             | `context/PreferencesContext.tsx`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | ~7           |
| Components with direct invoke | `LaunchPanel.tsx`, `pages/LaunchPage.tsx`, `pages/HealthDashboardPage.tsx`, `ProfileActions.tsx`, `TrainerDiscoveryPanel.tsx`, `OnboardingWizard.tsx`, `CommunityImportWizardModal.tsx`, `LauncherExport.tsx`, `SettingsPanel.tsx`, `SteamLaunchOptionsPanel.tsx`, `profile-sections/MediaSection.tsx`, `AutoPopulate.tsx`, `PrefixDepsPanel.tsx`                                                                                                                                                                                                                                                                                                                              | ~14          |
| Non-React utilities           | `utils/optimization-catalog.ts`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                | 1            |

### `listen` → `subscribeEvent` (13 files)

Import: `import { listen } from '@tauri-apps/api/event'` → `import { subscribeEvent } from '../lib/events'`

Cleanup pattern unchanged:

```ts
useEffect(() => {
  const p = subscribeEvent<T>('event-name', handler);
  return () => {
    p.then((f) => f());
  };
}, []);
```

Files: `App.tsx`, `context/ProfileContext.tsx`, `hooks/useProfile.ts`, `hooks/useProfileHealth.ts`, `hooks/useLaunchState.ts`, `hooks/useUpdateGame.ts`, `hooks/useRunExecutable.ts`, `hooks/useOfflineReadiness.ts`, `hooks/useCommunityProfiles.ts`, `components/layout/ConsoleDrawer.tsx`, `components/ConsoleView.tsx`, `components/PrefixDepsPanel.tsx`, `pages/LaunchPage.tsx`.

### Plugin imports (6 files via Vite alias, 2 files via import path change)

| Original import                              | Redirect           | Files                                                                                                    |
| -------------------------------------------- | ------------------ | -------------------------------------------------------------------------------------------------------- |
| `@tauri-apps/plugin-dialog`                  | Vite alias         | `utils/dialog.ts`, `components/CommunityBrowser.tsx`                                                     |
| `@tauri-apps/plugin-shell`                   | Vite alias         | `SettingsPanel.tsx`, `TrainerDiscoveryPanel.tsx`, `ExternalResultsSection.tsx`, `ProtonDbLookupCard.tsx` |
| `convertFileSrc` from `@tauri-apps/api/core` | Import path change | `hooks/useGameCoverArt.ts`, `components/profile-sections/MediaSection.tsx`                               |

---

## Data Models

### Four named states (controlled by `?fixture=`)

| State                 | Effect                                                                                                                                                        |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `populated` (default) | All list commands return ≥3 realistic entries; all scalar commands succeed                                                                                    |
| `empty`               | All list commands return `[]`; profile-dependent views show empty states                                                                                      |
| `error`               | Shell-critical reads (`settings_load`, `profile_list`) still succeed so app shell renders. Per-route IPC calls reject with `Error('Simulated backend error')` |
| `loading`             | All `callCommand` calls return a `Promise` that never resolves — simulates indefinite IO wait                                                                 |

Unknown `?fixture=` values silently fall back to `populated`.

### `?errors=true` (orthogonal write-error toggle)

Causes all commands NOT in `READ_COMMANDS` to reject. Read commands continue returning fixture data. The `READ_COMMANDS` set in `lib/mocks/index.ts` is the authoritative classification. This allows testing error banners overlaid on a realistic populated UI.

### `?delay=<ms>` (response latency simulation)

When non-zero, every `callCommand` response is deferred by `N` milliseconds. Applied uniformly by `wrapHandler`. Useful for verifying loading skeletons. Example: `?delay=800` simulates realistic Steam Deck IO latency.

### `?onboarding=show` (onboarding wizard opt-in)

The `onboarding-check` Tauri event never fires in browser mode. `SHOW_ONBOARDING` from the module-scope resolver triggers `setShowOnboarding(true)` on app mount in `App.tsx`. Without this toggle, the wizard is completely unreachable in browser mode.

---

## File Structure (new files)

```
src/crosshook-native/src/lib/
├── runtime.ts                          # isTauri() probe (6 lines, zero deps)
├── ipc.ts                              # callCommand<T>() adapter (~25 lines)
├── events.ts                           # subscribeEvent<T>() adapter + in-process bus (~35 lines)
├── plugin-stubs/
│   ├── dialog.ts                       # @tauri-apps/plugin-dialog: open/save → null
│   ├── shell.ts                        # @tauri-apps/plugin-shell: open → void
│   ├── event.ts                        # Vite alias target for @tauri-apps/api/event
│   └── convertFileSrc.ts              # (path: string) => path passthrough
└── mocks/
    ├── index.ts                        # registerMocks() + fixture-state resolver (module-scope)
    ├── store.ts                        # In-memory Map for round-trip state (profile saves, etc.)
    ├── README.md                       # How to add a new handler (worked example)
    └── handlers/
        ├── settings.ts                 # settings_load, recent_files_load, settings_save, recent_files_save, default_steam_client_install_path, settings_save_steamgriddb_key
        ├── profile.ts                  # profile_list, profile_load, profile_list_summaries, profile_list_favorites, profile_save, profile_delete, profile_duplicate, profile_rename, profile_set_favorite, profile_config_*, profile_save_launch_optimizations, profile_*_optimization_preset
        ├── launch.ts                   # validate_launch, launch_game, launch_trainer, preview_launch, check_version_status, check_gamescope_session, get_optimization_catalog, get_mangohud_presets, get_trainer_type_catalog
        ├── health.ts                   # batch_validate_profiles, get_profile_health, get_cached_health_snapshots, batch_offline_readiness, profile_mark_known_good, acknowledge_version_change, get_cached_offline_readiness_snapshots
        ├── install.ts                  # validate_install_request, install_game, detect_protontricks_binary, check_readiness, validate_run_executable_request, run_executable, cancel_run_executable, build_steam_launch_options_command
        ├── community.ts               # community_list_indexed_profiles, community_list_profiles, community_sync, community_add_tap, community_prepare_import, community_import_profile, community_export_profile
        ├── discovery.ts               # discovery_search_trainers
        ├── proton.ts                  # list_proton_installs, check_proton_migrations, apply_proton_migration
        ├── protondb.ts                # protondb_lookup, protondb_get_suggestions, protondb_accept_suggestion, protondb_dismiss_suggestion
        ├── protonup.ts                # protonup_list_available_versions, protonup_install_version, protonup_get_suggestion
        ├── launcher.ts                # list_launchers, check_launcher_exists, validate_launcher_export, export_launchers, preview_launcher_script, preview_launcher_desktop, delete_launcher
        ├── diagnostics.ts             # export_diagnostics
        ├── art.ts                     # fetch_game_cover_art, import_custom_art, fetch_game_metadata
        └── steam.ts                   # auto_populate_steam, build_steam_launch_options_command

src/crosshook-native/src/components/
└── DevModeChip.tsx                     # position: fixed badge (outside sidebar context)

src/crosshook-native/
└── .env.web-dev                        # VITE_WEB_DEV=true
```

---

## Edgecases

- **`convertFileSrc` is not an IPC command**: it is a synchronous function from `@tauri-apps/api/core`, not an `invoke()` command. The Vite alias does NOT cover it (aliasing all of `api/core` would break `invoke`). The two call sites must update their import explicitly to `lib/plugin-stubs/convertFileSrc.ts`.

- **`utils/optimization-catalog.ts` calls `invoke` directly in a non-React module**: it caches the catalog in a module-level ref. It is in migration scope. Import path change: `@tauri-apps/api/core` → `../lib/ipc`.

- **`profile_list_summaries` type `ProfileSummary` is not exported**: the interface is local to `hooks/useLibrarySummaries.ts`. Move to `types/library.ts` before writing a typed mock. Use a type assertion in the handler until that migration is done.

- **The `loading` fixture creates permanently pending Promises**: components with `Promise.race` or timeout logic may behave unexpectedly. Document in `lib/mocks/README.md`.

- **Vite HMR resets the mock singleton**: when any file in `lib/mocks/` is edited, HMR invalidates the module graph. The `mocksPromise` latch in `ipc.ts` is reset. The in-memory `store.ts` state is also reset — contributors editing profile-mutation handlers should expect in-session saves to be lost on hot reload.

- **In-process event bus is global**: `browserBus` in `lib/events.ts` is module-level state. Multiple components subscribing to the same event name all receive the event — matching real Tauri behavior.

- **`DevModeChip` imports from `lib/mocks/index.ts`**: in Tauri mode, `isTauri()` returns `true` and the chip renders `null` — the constants are never read, but the module-scope resolver code in `mocks/index.ts` does execute on import. `URLSearchParams(window.location.search)` is safe in Tauri mode. If bundle analysis ever shows the mocks module bleeding into production, replace the direct import with a `window.__CROSSHOOK_FIXTURE__` global set by the mocks module at init time.

- **`server.host` is already loopback when `TAURI_DEV_HOST` is unset**: the existing vite config already restricts to loopback in plain `vite` mode. The `web-dev` mode makes this explicit with `'127.0.0.1'`.

- **`DevModeChip` needs no scroll or layout registration**: it uses `position: fixed`, which takes it out of normal flow. No `useScrollEnhance` selector registration is required (AGENTS.md rule: only new `overflow-y: auto` containers need registration).

---

## Full IPC Inventory

Authoritative enumeration of every Tauri command registered in
`src/crosshook-native/src-tauri/src/lib.rs:208-330` (the single
`invoke_handler![…]` macro), cross-referenced against every `invoke<T>(...)`
and `listen<T>(...)` call site in `src/crosshook-native/src/`. Grouped by
Rust command module to keep mock files 1:1 with the backend layout.

**Complexity**: **S** = static fixture (no-op or return constant), **M** =
moderate (shape-matching, deterministic variation by args), **L** = large
(synthesizes progress events over time, multi-stage).

**Totals**: 119 commands called from frontend code today, plus 9
`lib.rs`-registered commands with no current frontend call site (landing
pads). 16 distinct `listen()` event names.

### settings (5)

| Command                         | Call sites                                                                                                         | Rust handler           | Return            | Complexity |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ---------------------- | ----------------- | ---------- |
| `settings_load`                 | `context/PreferencesContext.tsx:44, 107`; `hooks/useProfile.ts:509, 630`; `hooks/useCommunityProfiles.ts:228, 392` | `commands/settings.rs` | `AppSettingsData` | S          |
| `settings_save`                 | `context/PreferencesContext.tsx:106`; `hooks/useProfile.ts:510, 632`; `hooks/useCommunityProfiles.ts:229`          | `commands/settings.rs` | `void`            | S          |
| `settings_save_steamgriddb_key` | `context/PreferencesContext.tsx:135`                                                                               | `commands/settings.rs` | `void`            | S          |
| `recent_files_load`             | `context/PreferencesContext.tsx:45`; `hooks/useProfile.ts:514`                                                     | `commands/settings.rs` | `RecentFilesData` | S          |
| `recent_files_save`             | `context/PreferencesContext.tsx:157`; `hooks/useProfile.ts:515`                                                    | `commands/settings.rs` | `void`            | S          |

### profile (22)

| Command                                     | Call sites                                                                                        | Rust handler          | Return                             | Complexity |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------- | --------------------- | ---------------------------------- | ---------- |
| `profile_list`                              | `hooks/useProfile.ts:602, 636`; `hooks/useUpdateGame.ts:115`; `hooks/useCommunityProfiles.ts:240` | `commands/profile.rs` | `string[]`                         | S          |
| `profile_list_summaries`                    | `hooks/useLibrarySummaries.ts:42`                                                                 | `commands/profile.rs` | `ProfileSummary[]`                 | M          |
| `profile_list_favorites`                    | `hooks/useProfile.ts:529`                                                                         | `commands/profile.rs` | `string[]`                         | S          |
| `profile_load`                              | `hooks/useProfile.ts:567`; `hooks/useUpdateGame.ts:121, 148`; `hooks/useGameDetailsProfile.ts:47` | `commands/profile.rs` | `SerializedGameProfile`            | M          |
| `profile_save`                              | `hooks/useProfile.ts:1045`; `hooks/useCommunityProfiles.ts:372`                                   | `commands/profile.rs` | `void`                             | S          |
| `profile_set_favorite`                      | `hooks/useProfile.ts:539`                                                                         | `commands/profile.rs` | `void`                             | S          |
| `profile_delete`                            | `hooks/useProfile.ts:1188`                                                                        | `commands/profile.rs` | `void`                             | S          |
| `profile_duplicate`                         | `hooks/useProfile.ts:1082`                                                                        | `commands/profile.rs` | `SerializedDuplicateProfileResult` | M          |
| `profile_rename`                            | `hooks/useProfile.ts:1133`                                                                        | `commands/profile.rs` | `boolean`                          | S          |
| `profile_save_launch_optimizations`         | `hooks/useProfile.ts:730, 830, 1357`                                                              | `commands/profile.rs` | `void`                             | S          |
| `profile_save_mangohud_config`              | `hooks/useProfile.ts:1543`                                                                        | `commands/profile.rs` | `void`                             | S          |
| `profile_save_gamescope_config`             | `hooks/useProfile.ts:1429`                                                                        | `commands/profile.rs` | `void`                             | S          |
| `profile_save_trainer_gamescope_config`     | `hooks/useProfile.ts:1486`                                                                        | `commands/profile.rs` | `void`                             | S          |
| `profile_list_bundled_optimization_presets` | `hooks/useProfile.ts:1290`                                                                        | `commands/profile.rs` | `BundledOptimizationPreset[]`      | S          |
| `profile_apply_bundled_optimization_preset` | `hooks/useProfile.ts:912`                                                                         | `commands/profile.rs` | `SerializedGameProfile`            | M          |
| `profile_save_manual_optimization_preset`   | `hooks/useProfile.ts:995`                                                                         | `commands/profile.rs` | `SerializedGameProfile`            | M          |
| `profile_export_toml`                       | `components/pages/ProfilesPage.tsx:448`                                                           | `commands/profile.rs` | `string`                           | S          |
| `profile_config_history`                    | `hooks/useProfile.ts:1205`                                                                        | `commands/profile.rs` | `ConfigRevisionSummary[]`          | S          |
| `profile_config_diff`                       | `hooks/useProfile.ts:1223`                                                                        | `commands/profile.rs` | `ConfigDiffResult`                 | M          |
| `profile_config_rollback`                   | `hooks/useProfile.ts:1244`                                                                        | `commands/profile.rs` | `ConfigRollbackResult`             | M          |
| `profile_mark_known_good`                   | `hooks/useProfile.ts:1269`                                                                        | `commands/profile.rs` | `void`                             | S          |
| `profile_import_legacy`                     | (registered, no current frontend call site)                                                       | `commands/profile.rs` | structured                         | S          |

### launch (7)

| Command                              | Call sites                                                                     | Rust handler         | Return          | Complexity                                                         |
| ------------------------------------ | ------------------------------------------------------------------------------ | -------------------- | --------------- | ------------------------------------------------------------------ |
| `launch_game`                        | `hooks/useLaunchState.ts:284`                                                  | `commands/launch.rs` | `LaunchResult`  | **L** (emits `launch-log`, `launch-diagnostic`, `launch-complete`) |
| `launch_trainer`                     | `hooks/useLaunchState.ts:362`                                                  | `commands/launch.rs` | `LaunchResult`  | **L** (same event stream)                                          |
| `validate_launch`                    | `hooks/useLaunchState.ts:129`; `components/CommunityImportWizardModal.tsx:661` | `commands/launch.rs` | `void`          | S                                                                  |
| `preview_launch`                     | `hooks/usePreviewState.ts:16`; `components/CommunityImportWizardModal.tsx:312` | `commands/launch.rs` | `LaunchPreview` | M                                                                  |
| `build_steam_launch_options_command` | `components/SteamLaunchOptionsPanel.tsx:48`                                    | `commands/launch.rs` | `string`        | S                                                                  |
| `check_gamescope_session`            | `components/pages/LaunchPage.tsx:37`                                           | `commands/launch.rs` | `boolean`       | S                                                                  |
| `check_game_running`                 | `hooks/useLaunchState.ts:211`                                                  | `commands/launch.rs` | `boolean`       | S                                                                  |

### install (3)

| Command                       | Call sites                    | Rust handler          | Return              | Complexity |
| ----------------------------- | ----------------------------- | --------------------- | ------------------- | ---------- |
| `install_game`                | `hooks/useInstallGame.ts:295` | `commands/install.rs` | `InstallGameResult` | **L**      |
| `install_default_prefix_path` | `hooks/useInstallGame.ts:220` | `commands/install.rs` | `string`            | S          |
| `validate_install_request`    | `hooks/useInstallGame.ts:289` | `commands/install.rs` | `void`              | S          |

### update (3)

| Command                   | Call sites                   | Rust handler         | Return             | Complexity                                    |
| ------------------------- | ---------------------------- | -------------------- | ------------------ | --------------------------------------------- |
| `update_game`             | `hooks/useUpdateGame.ts:251` | `commands/update.rs` | `UpdateGameResult` | **L** (emits `update-log`, `update-complete`) |
| `validate_update_request` | `hooks/useUpdateGame.ts:200` | `commands/update.rs` | `void`             | S                                             |
| `cancel_update`           | `hooks/useUpdateGame.ts:268` | `commands/update.rs` | `void`             | S                                             |

### run_executable (4)

| Command                           | Call sites                      | Rust handler                 | Return                | Complexity                              |
| --------------------------------- | ------------------------------- | ---------------------------- | --------------------- | --------------------------------------- |
| `run_executable`                  | `hooks/useRunExecutable.ts:200` | `commands/run_executable.rs` | `RunExecutableResult` | **L** (emits `run-executable-complete`) |
| `validate_run_executable_request` | `hooks/useRunExecutable.ts:149` | `commands/run_executable.rs` | `void`                | S                                       |
| `cancel_run_executable`           | `hooks/useRunExecutable.ts:127` | `commands/run_executable.rs` | `void`                | S                                       |
| `stop_run_executable`             | `hooks/useRunExecutable.ts:135` | `commands/run_executable.rs` | `void`                | S                                       |

### health (4)

| Command                                  | Call sites                                                    | Rust handler         | Return                             | Complexity                                |
| ---------------------------------------- | ------------------------------------------------------------- | -------------------- | ---------------------------------- | ----------------------------------------- |
| `batch_validate_profiles`                | `hooks/useProfileHealth.ts:123`; `hooks/useOnboarding.ts:156` | `commands/health.rs` | `EnrichedHealthSummary`            | M (emits `profile-health-batch-complete`) |
| `get_profile_health`                     | `hooks/useProfileHealth.ts:139`                               | `commands/health.rs` | `EnrichedProfileHealthReport`      | M                                         |
| `get_cached_health_snapshots`            | `hooks/useProfileHealth.ts:161`                               | `commands/health.rs` | `CachedHealthSnapshot[]`           | S                                         |
| `get_cached_offline_readiness_snapshots` | `hooks/useOfflineReadiness.ts:130`                            | `commands/health.rs` | `CachedOfflineReadinessSnapshot[]` | S                                         |

### offline (5)

| Command                    | Call sites                                                             | Rust handler          | Return                     | Complexity                                  |
| -------------------------- | ---------------------------------------------------------------------- | --------------------- | -------------------------- | ------------------------------------------- |
| `check_offline_readiness`  | `hooks/useOfflineReadiness.ts:108`; `hooks/useLaunchState.ts:173, 322` | `commands/offline.rs` | `OfflineReadinessReport`   | M                                           |
| `batch_offline_readiness`  | `hooks/useOfflineReadiness.ts:91`                                      | `commands/offline.rs` | `OfflineReadinessReport[]` | M (emits `offline-readiness-scan-complete`) |
| `verify_trainer_hash`      | `hooks/useLaunchState.ts:389`                                          | `commands/offline.rs` | `HashVerifyResult`         | S                                           |
| `check_network_status`     | (registered; referenced via health flow)                               | `commands/offline.rs` | structured                 | S                                           |
| `get_trainer_type_catalog` | `hooks/useTrainerTypeCatalog.ts:12`                                    | `commands/offline.rs` | `TrainerTypeEntry[]`       | S                                           |

### onboarding (3)

| Command                | Call sites                                  | Rust handler             | Return                   | Complexity |
| ---------------------- | ------------------------------------------- | ------------------------ | ------------------------ | ---------- |
| `check_readiness`      | `hooks/useOnboarding.ts:116`                | `commands/onboarding.rs` | `ReadinessCheckResult`   | M          |
| `dismiss_onboarding`   | `hooks/useOnboarding.ts:152`                | `commands/onboarding.rs` | `void`                   | S          |
| `get_trainer_guidance` | (registered, no current frontend call site) | `commands/onboarding.rs` | `TrainerGuidanceContent` | S          |

### catalog (2)

| Command                    | Call sites                         | Rust handler          | Return                       | Complexity |
| -------------------------- | ---------------------------------- | --------------------- | ---------------------------- | ---------- |
| `get_optimization_catalog` | `utils/optimization-catalog.ts:34` | `commands/catalog.rs` | `OptimizationCatalogPayload` | S          |
| `get_mangohud_presets`     | `hooks/useMangoHudPresets.ts:33`   | `commands/catalog.rs` | `MangoHudPreset[]`           | S          |

### community (7)

| Command                           | Call sites                                 | Rust handler            | Return                         | Complexity                   |
| --------------------------------- | ------------------------------------------ | ----------------------- | ------------------------------ | ---------------------------- |
| `community_list_profiles`         | `hooks/useCommunityProfiles.ts:235`        | `commands/community.rs` | `CommunityProfileIndex`        | M                            |
| `community_list_indexed_profiles` | `components/pages/ProfilesPage.tsx:224`    | `commands/community.rs` | `CommunityIndexedProfileRow[]` | M                            |
| `community_add_tap`               | `hooks/useCommunityProfiles.ts:273`        | `commands/community.rs` | `CommunityTapSubscription[]`   | S                            |
| `community_sync`                  | `hooks/useCommunityProfiles.ts:249`        | `commands/community.rs` | `CommunityTapSyncResult[]`     | M (emits `profiles-changed`) |
| `community_prepare_import`        | `hooks/useCommunityProfiles.ts:355`        | `commands/community.rs` | `CommunityImportPreview`       | M                            |
| `community_import_profile`        | `components/TrainerDiscoveryPanel.tsx:248` | `commands/community.rs` | `void`                         | S                            |
| `community_export_profile`        | `components/pages/ProfilesPage.tsx:428`    | `commands/community.rs` | `CommunityExportResult`        | S                            |

### collections (6)

| Command                     | Call sites                                  | Rust handler              | Return         | Complexity |
| --------------------------- | ------------------------------------------- | ------------------------- | -------------- | ---------- |
| `collection_list`           | (registered, no current frontend call site) | `commands/collections.rs` | `Collection[]` | S          |
| `collection_create`         | (same)                                      | `commands/collections.rs` | `Collection`   | S          |
| `collection_delete`         | (same)                                      | `commands/collections.rs` | `void`         | S          |
| `collection_add_profile`    | (same)                                      | `commands/collections.rs` | `void`         | S          |
| `collection_remove_profile` | (same)                                      | `commands/collections.rs` | `void`         | S          |
| `collection_list_profiles`  | (same)                                      | `commands/collections.rs` | `string[]`     | S          |

### prefix_deps (4)

| Command                      | Call sites                                                              | Rust handler              | Return                           | Complexity                       |
| ---------------------------- | ----------------------------------------------------------------------- | ------------------------- | -------------------------------- | -------------------------------- |
| `detect_protontricks_binary` | `components/SettingsPanel.tsx:881`                                      | `commands/prefix_deps.rs` | `{ found, binary_name, source }` | S                                |
| `check_prefix_dependencies`  | `hooks/usePrefixDeps.ts:40`                                             | `commands/prefix_deps.rs` | `PrefixDependencyStatus[]`       | M                                |
| `install_prefix_dependency`  | `hooks/usePrefixDeps.ts:86`; `components/pages/LaunchPage.tsx:182, 416` | `commands/prefix_deps.rs` | `void`                           | M (emits `prefix-deps-progress`) |
| `get_dependency_status`      | `hooks/usePrefixDeps.ts:68`; `components/pages/LaunchPage.tsx:164`      | `commands/prefix_deps.rs` | `PrefixDependencyStatus[]`       | S                                |

### storage (3)

| Command                      | Call sites                               | Rust handler          | Return                         | Complexity |
| ---------------------------- | ---------------------------------------- | --------------------- | ------------------------------ | ---------- |
| `get_prefix_storage_history` | `hooks/usePrefixStorageManagement.ts:51` | `commands/storage.rs` | `PrefixStorageHistoryResponse` | S          |
| `scan_prefix_storage`        | `hooks/usePrefixStorageManagement.ts:68` | `commands/storage.rs` | `PrefixStorageScanResult`      | M          |
| `cleanup_prefix_storage`     | `hooks/usePrefixStorageManagement.ts:85` | `commands/storage.rs` | `PrefixCleanupResult`          | M          |

### steam (3)

| Command                             | Call sites                                                                                                           | Rust handler        | Return                    | Complexity |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------- | ------------------------- | ---------- |
| `auto_populate_steam`               | `components/AutoPopulate.tsx:136`; `components/CommunityImportWizardModal.tsx:239`                                   | `commands/steam.rs` | `SteamAutoPopulateResult` | M          |
| `default_steam_client_install_path` | `context/PreferencesContext.tsx:46`; `components/CommunityImportWizardModal.tsx:211`                                 | `commands/steam.rs` | `string`                  | S          |
| `list_proton_installs`              | `hooks/useProtonInstalls.ts:45`; `components/OnboardingWizard.tsx:164`; `components/pages/ProfilesPage.tsx:182, 809` | `commands/steam.rs` | `ProtonInstallOption[]`   | S          |

### game_metadata (4)

| Command                   | Call sites                                        | Rust handler                | Return                      | Complexity |
| ------------------------- | ------------------------------------------------- | --------------------------- | --------------------------- | ---------- | --- |
| `fetch_game_metadata`     | `hooks/useGameMetadata.ts:108`                    | `commands/game_metadata.rs` | `SteamMetadataLookupResult` | M          |
| `fetch_game_cover_art`    | `hooks/useGameCoverArt.ts:43`                     | `commands/game_metadata.rs` | `string                     | null`      | S   |
| `import_custom_cover_art` | (registered, no current frontend call site)       | `commands/game_metadata.rs` | `string`                    | S          |
| `import_custom_art`       | `components/profile-sections/MediaSection.tsx:58` | `commands/game_metadata.rs` | `string`                    | S          |

### protondb (4)

| Command                       | Call sites                           | Rust handler           | Return                   | Complexity |
| ----------------------------- | ------------------------------------ | ---------------------- | ------------------------ | ---------- |
| `protondb_lookup`             | `hooks/useProtonDbLookup.ts:106`     | `commands/protondb.rs` | `ProtonDbLookupResult`   | M          |
| `protondb_get_suggestions`    | `hooks/useProtonDbSuggestions.ts:42` | `commands/protondb.rs` | `ProtonDbSuggestionSet`  | M          |
| `protondb_accept_suggestion`  | `hooks/useProtonDbSuggestions.ts:83` | `commands/protondb.rs` | `AcceptSuggestionResult` | S          |
| `protondb_dismiss_suggestion` | `hooks/useProtonDbSuggestions.ts:94` | `commands/protondb.rs` | `void`                   | S          |

### protonup (3)

| Command                            | Call sites                    | Rust handler           | Return                    | Complexity |
| ---------------------------------- | ----------------------------- | ---------------------- | ------------------------- | ---------- |
| `protonup_list_available_versions` | `hooks/useProtonUp.ts:60, 97` | `commands/protonup.rs` | `ProtonUpCatalogResponse` | M          |
| `protonup_install_version`         | `hooks/useProtonUp.ts:142`    | `commands/protonup.rs` | `ProtonUpInstallResult`   | **L**      |
| `protonup_get_suggestion`          | `hooks/useProtonUp.ts:162`    | `commands/protonup.rs` | `ProtonUpSuggestion`      | S          |

### migration (3)

| Command                   | Call sites                       | Rust handler            | Return                 | Complexity |
| ------------------------- | -------------------------------- | ----------------------- | ---------------------- | ---------- |
| `check_proton_migrations` | `hooks/useProtonMigration.ts:27` | `commands/migration.rs` | `MigrationScanResult`  | M          |
| `apply_proton_migration`  | `hooks/useProtonMigration.ts:45` | `commands/migration.rs` | `MigrationApplyResult` | M          |
| `apply_batch_migration`   | `hooks/useProtonMigration.ts:66` | `commands/migration.rs` | `BatchMigrationResult` | M          |

### discovery (6)

| Command                                 | Call sites                             | Rust handler            | Return                          | Complexity |
| --------------------------------------- | -------------------------------------- | ----------------------- | ------------------------------- | ---------- |
| `discovery_search_trainers`             | `hooks/useTrainerDiscovery.ts:43`      | `commands/discovery.rs` | `TrainerSearchResponse`         | M          |
| `discovery_search_external`             | `hooks/useExternalTrainerSearch.ts:53` | `commands/discovery.rs` | `ExternalTrainerSearchResponse` | M          |
| `discovery_check_version_compatibility` | (registered, wiring pending)           | `commands/discovery.rs` | structured                      | S          |
| `discovery_list_external_sources`       | (same)                                 | `commands/discovery.rs` | structured                      | S          |
| `discovery_add_external_source`         | (same)                                 | `commands/discovery.rs` | structured                      | S          |
| `discovery_remove_external_source`      | (same)                                 | `commands/discovery.rs` | `void`                          | S          |

### export — launchers (12)

| Command                      | Call sites                                  | Rust handler         | Return                              | Complexity |
| ---------------------------- | ------------------------------------------- | -------------------- | ----------------------------------- | ---------- |
| `list_launchers`             | `hooks/useLauncherManagement.ts:34`         | `commands/export.rs` | `LauncherInfo[]`                    | S          |
| `delete_launcher_by_slug`    | `hooks/useLauncherManagement.ts:52`         | `commands/export.rs` | `LauncherDeleteResult`              | S          |
| `delete_launcher`            | `components/LauncherExport.tsx:336`         | `commands/export.rs` | `LauncherDeleteResult`              | S          |
| `reexport_launcher_by_slug`  | `hooks/useLauncherManagement.ts:74`         | `commands/export.rs` | `void`                              | S          |
| `export_launchers`           | `components/LauncherExport.tsx:188, 275`    | `commands/export.rs` | `SteamExternalLauncherExportResult` | M          |
| `validate_launcher_export`   | `components/LauncherExport.tsx:187, 274`    | `commands/export.rs` | `void`                              | S          |
| `check_launcher_exists`      | `components/LauncherExport.tsx:155`         | `commands/export.rs` | `LauncherInfo`                      | S          |
| `check_launcher_for_profile` | `hooks/useProfile.ts:1161`                  | `commands/export.rs` | `LauncherInfo`                      | S          |
| `preview_launcher_script`    | `components/LauncherExport.tsx:318`         | `commands/export.rs` | `string`                            | S          |
| `preview_launcher_desktop`   | `components/LauncherExport.tsx:319`         | `commands/export.rs` | `string`                            | S          |
| `find_orphaned_launchers`    | (registered, no current frontend call site) | `commands/export.rs` | structured                          | S          |
| `rename_launcher`            | (same)                                      | `commands/export.rs` | `void`                              | S          |

### version (4)

| Command                      | Call sites                                                                   | Rust handler          | Return               | Complexity |
| ---------------------------- | ---------------------------------------------------------------------------- | --------------------- | -------------------- | ---------- |
| `check_version_status`       | `hooks/useOnboarding.ts:159`; `components/pages/HealthDashboardPage.tsx:873` | `commands/version.rs` | `VersionCheckResult` | M          |
| `get_version_snapshot`       | (registered, no current frontend call site)                                  | `commands/version.rs` | structured           | S          |
| `set_trainer_version`        | `hooks/useSetTrainerVersion.ts:32`                                           | `commands/version.rs` | `void`               | S          |
| `acknowledge_version_change` | `components/ProfileActions.tsx:102`; `components/LaunchPanel.tsx:643`        | `commands/version.rs` | `void`               | S          |

### diagnostics (1)

| Command              | Call sites                         | Rust handler              | Return                   | Complexity |
| -------------------- | ---------------------------------- | ------------------------- | ------------------------ | ---------- |
| `export_diagnostics` | `components/SettingsPanel.tsx:606` | `commands/diagnostics.rs` | `DiagnosticBundleResult` | S          |

### Event channels (16 distinct `listen(...)` names)

Sourced from the 18 `listen<T>('event-name', ...)` call sites. These need a
no-op subscribe path in browser mode (`lib/events.ts`'s `subscribeEvent`),
and the action-command mock handlers drive them via `emitMockEvent`.

| Event                             | Call sites                                                                                       | Typical payload                       |
| --------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------- | ----- |
| `auto-load-profile`               | `context/ProfileContext.tsx:36`                                                                  | `string`                              |
| `onboarding-check`                | `App.tsx:67`                                                                                     | `OnboardingCheckPayload`              |
| `profiles-changed`                | `hooks/useProfile.ts:1307`; `hooks/useCommunityProfiles.ts:425`; `hooks/useProfileHealth.ts:194` | `string`                              |
| `launch-log`                      | `components/ConsoleView.tsx:65`; `components/layout/ConsoleDrawer.tsx:74`                        | `LogPayload`                          |
| `update-log`                      | `components/ConsoleView.tsx:66`; `components/layout/ConsoleDrawer.tsx:75`                        | `LogPayload`                          |
| `launch-diagnostic`               | `hooks/useLaunchState.ts:231`                                                                    | `DiagnosticReport`                    |
| `launch-complete`                 | `hooks/useProfileHealth.ts:198`                                                                  | `unknown`                             |
| `update-complete`                 | `hooks/useUpdateGame.ts:232`                                                                     | `number                               | null` |
| `run-executable-complete`         | `hooks/useRunExecutable.ts:186`                                                                  | `number                               | null` |
| `profile-health-batch-complete`   | `hooks/useProfileHealth.ts:152`                                                                  | `EnrichedHealthSummary`               |
| `offline-readiness-scan-complete` | `hooks/useOfflineReadiness.ts:121`                                                               | `OfflineReadinessScanCompletePayload` |
| `version-scan-complete`           | `hooks/useProfileHealth.ts:202`                                                                  | `unknown`                             |
| `prefix-deps-progress`            | `components/PrefixDepsPanel.tsx`                                                                 | structured                            |
| `install-log`                     | (emitted by Rust during install; frontend listener via ConsoleView)                              | `LogPayload`                          |
| `install-complete`                | (emitted by Rust during install)                                                                 | `number                               | null` |
| `protonup-install-progress`       | (emitted during ProtonUp install)                                                                | structured                            |

---

## Tree-shaking Strategy

Restated as a top-level section for easy reference. Full detail is in the
"Tree-Shaking & Build-Time Elimination" subsection of Architecture Design.

Goals (in priority order):

1. **Zero mock bytes in the production AppImage.** Verified by the CI
   sentinel grep on `src/crosshook-native/dist/` for `registerMocks`,
   `lib/mocks`, `[mock] callCommand`, and `MOCK MODE` strings. Any hit is a
   build failure.
2. **Zero mock imports in the production `tsc` output.** Verified by an
   ESLint `no-restricted-imports` rule forbidding any file outside
   `src/lib/ipc.ts` and `src/lib/events.ts` from importing
   `src/lib/mocks/**`.
3. **Zero mock runtime evaluation in Tauri mode.** Verified by the
   `isTauri()` runtime gate — when true, the mock dynamic import never
   executes, regardless of whether Rollup managed to statically eliminate
   the chunk.

Mechanisms:

- `lib/ipc.ts` uses `if (isTauri()) { ... } else { ensureMocks() }`. The
  mock dynamic import is only reachable on the else branch.
- `lib/events.ts` uses the same pattern for the `listen`/mock-bus split.
- All mock fixture data is confined to `src/lib/mocks/**`. Fixture modules
  are never imported directly by components, hooks, or contexts.
- An optional `__WEB_DEV_MODE__` `define` (Option 2 in Architecture Design)
  is available as a hardening step if the CI sentinel ever trips. This
  converts the runtime `isTauri()` gate into a compile-time literal that
  Rollup can eliminate at chunk-graph construction, not just minification.
  `import.meta.env.MODE === 'webdev'` (with `npm run dev:browser` passing
  `--mode webdev`) is an equivalent alternative with lower misconfig risk.
- The CI sentinel (`.github/workflows/release.yml`) is the single source of
  truth — no assumption about Rollup behavior is relied upon without it.
- An ESLint override on `src/lib/mocks/**` forbids `fetch`,
  `XMLHttpRequest`, `WebSocket`, `EventSource`, `Worker`, `localStorage`,
  `sessionStorage`, `indexedDB` to enforce the "static fixture only" posture
  on mock handlers.

Failure modes and detection:

- **A new static import of `lib/mocks/**` slips into production code\*\* →
  ESLint catches it at author time; failing that, the CI sentinel catches
  it at build time.
- **A fixture file accidentally imports `@tauri-apps/api/core`** → no-op in
  browser mode (dynamic import resolves), but it should never happen;
  ESLint `no-restricted-imports` can block it specifically.
- **Rollup decides not to prune the dead `import('./mocks/index')` chunk**
  → CI sentinel still fails the build if the chunk content contains any
  sentinel string, because the chunk would still be written to `dist/`.
  Hardening path: switch the runtime `isTauri()` gate to
  `__WEB_DEV_MODE__` compile-time constant.

---

## Files to Create

Authoritative, structured list. Several already appear inline under
"Relevant Files"; this section provides the deliverable checklist for
implementation PRs.

### Core adapter

1. `src/crosshook-native/src/lib/runtime.ts` — `isTauri()` probe, no
   dependencies.
2. `src/crosshook-native/src/lib/ipc.ts` — `callCommand<T>` adapter.
3. `src/crosshook-native/src/lib/events.ts` — `subscribeEvent<T>` +
   `emitMockEvent` for mock handlers.
4. `src/crosshook-native/src/lib/DevModeBanner.tsx` — conditional banner.

### Plugin stubs

5. `src/crosshook-native/src/lib/plugin-stubs/dialog.ts`
6. `src/crosshook-native/src/lib/plugin-stubs/shell.ts`
7. `src/crosshook-native/src/lib/plugin-stubs/fs.ts`
8. `src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts` (plus
   `initConverters()` called from `main.tsx`)

### Mock registry

9. `src/crosshook-native/src/lib/mocks/index.ts` — `registerMocks()`
   assembly + `MOCK_FIXTURE_MARKER` sentinel constant.
10. `src/crosshook-native/src/lib/mocks/store.ts` — in-memory mutable state.
11. `src/crosshook-native/src/lib/mocks/eventBus.ts` — wiring between mock
    handlers and `lib/events.ts` bus.
12. `src/crosshook-native/src/lib/mocks/README.md` — "how to add a handler"
    with one worked example.

### Fixture data (one per domain, colocated with handlers)

Fixtures are committed TypeScript modules. They import from
`src/crosshook-native/src/types/*` so a backend type rename breaks the
fixture at `tsc --noEmit` time.

13. `src/crosshook-native/src/lib/mocks/fixtures/settings.ts`
14. `src/crosshook-native/src/lib/mocks/fixtures/profiles.ts`
15. `src/crosshook-native/src/lib/mocks/fixtures/catalogs.ts`
16. `src/crosshook-native/src/lib/mocks/fixtures/health.ts`
17. `src/crosshook-native/src/lib/mocks/fixtures/offline.ts`
18. `src/crosshook-native/src/lib/mocks/fixtures/proton.ts`
19. `src/crosshook-native/src/lib/mocks/fixtures/community.ts`
20. `src/crosshook-native/src/lib/mocks/fixtures/launchers.ts`
21. `src/crosshook-native/src/lib/mocks/fixtures/discovery.ts`
22. `src/crosshook-native/src/lib/mocks/fixtures/prefix.ts`
23. `src/crosshook-native/src/lib/mocks/fixtures/migration.ts`
24. `src/crosshook-native/src/lib/mocks/fixtures/diagnostics.ts`
25. `src/crosshook-native/src/lib/mocks/fixtures/system.ts` (gamescope,
    network, version stubs)

### Mock handlers (1:1 with `src-tauri/src/commands/*.rs` modules)

26. `src/crosshook-native/src/lib/mocks/handlers/settings.ts`
27. `src/crosshook-native/src/lib/mocks/handlers/profile.ts`
28. `src/crosshook-native/src/lib/mocks/handlers/launch.ts`
29. `src/crosshook-native/src/lib/mocks/handlers/install.ts`
30. `src/crosshook-native/src/lib/mocks/handlers/update.ts`
31. `src/crosshook-native/src/lib/mocks/handlers/run_executable.ts`
32. `src/crosshook-native/src/lib/mocks/handlers/health.ts`
33. `src/crosshook-native/src/lib/mocks/handlers/catalog.ts`
34. `src/crosshook-native/src/lib/mocks/handlers/offline.ts`
35. `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts`
36. `src/crosshook-native/src/lib/mocks/handlers/community.ts`
37. `src/crosshook-native/src/lib/mocks/handlers/collections.ts`
38. `src/crosshook-native/src/lib/mocks/handlers/prefix_deps.ts`
39. `src/crosshook-native/src/lib/mocks/handlers/protondb.ts`
40. `src/crosshook-native/src/lib/mocks/handlers/protonup.ts`
41. `src/crosshook-native/src/lib/mocks/handlers/migration.ts`
42. `src/crosshook-native/src/lib/mocks/handlers/export.ts`
43. `src/crosshook-native/src/lib/mocks/handlers/steam.ts`
44. `src/crosshook-native/src/lib/mocks/handlers/storage.ts`
45. `src/crosshook-native/src/lib/mocks/handlers/discovery.ts`
46. `src/crosshook-native/src/lib/mocks/handlers/game_metadata.ts`
47. `src/crosshook-native/src/lib/mocks/handlers/version.ts`
48. `src/crosshook-native/src/lib/mocks/handlers/diagnostics.ts`

### Test coverage

49. `src/crosshook-native/src/lib/__tests__/ipc.test.ts` — unit tests for
    runtime detection + handler dispatch + missing-handler error shape.
50. `src/crosshook-native/src/lib/__tests__/events.test.ts` — subscribe /
    emit / unlisten round-trip.
51. `src/crosshook-native/src/lib/mocks/__tests__/registry.test.ts` —
    smoke test comparing `registerMocks()` keys against the command list
    extracted from `src-tauri/src/lib.rs` at test time. Fails on drift in
    either direction (new backend command without mock, stale mock without
    backend).

### Optional artifacts

52. `src/crosshook-native/public/mock-art/` — placeholder cover PNGs
    keyed by synthetic appIds for the `fetch_game_cover_art` mock.
53. `src/crosshook-native/src/components/dev/WebDevOverlay.tsx` — optional
    Phase 4 dev badge surfacing unhandled-command errors (superset of
    `DevModeBanner`).

## Files to Modify

### Scripts and config

1. `scripts/dev-native.sh` — add `--browser` / `--web` branch.
2. `src/crosshook-native/package.json` — add `"dev:browser": "vite"` (or
   `"vite --mode webdev"` if the hardened `define`-based gate is adopted).
3. `src/crosshook-native/vite.config.ts` — add optional `@/` path alias
   and optional `define: { __WEB_DEV_MODE__ }`.
4. `src/crosshook-native/src/vite-env.d.ts` — declare `__WEB_DEV_MODE__` if
   the hardened gate is adopted; otherwise no change.
5. `src/crosshook-native/tsconfig.json` — add matching `paths` for the
   `@/` alias if that alias is adopted.
6. `.github/workflows/release.yml` — add CI sentinel grep after the
   AppImage build step.
7. `.eslintrc.cjs` (create or extend) — add `no-restricted-imports` rule
   restricting `lib/mocks/**` imports, and `no-restricted-globals` override
   on `lib/mocks/**`.

### Boot and banner

8. `src/crosshook-native/src/main.tsx` — `await initConverters()` before
   `ReactDOM.createRoot(...)`.
9. `src/crosshook-native/src/App.tsx` — render `<DevModeBanner />` above
   `<ProfileProvider>`; migrate the direct `listen('onboarding-check', …)`
   call to `subscribeEvent`.

### Callsite migration (mechanical: `invoke(` → `callCommand(`, `listen(` → `subscribeEvent(`)

The following files contain one or more direct `invoke(...)` or
`listen(...)` calls from `@tauri-apps/api/{core,event}` and require import
rewrites. Derived from the Grep sweep referenced in the IPC inventory.

Context and utils:

10. `src/crosshook-native/src/context/PreferencesContext.tsx` (most
    critical: 3-command parallel boot sequence).
11. `src/crosshook-native/src/context/ProfileContext.tsx`
12. `src/crosshook-native/src/utils/optimization-catalog.ts` (module-level
    cache, not in a React hook — same migration).

Hooks:

13. `src/crosshook-native/src/hooks/useLibrarySummaries.ts`
14. `src/crosshook-native/src/hooks/useTrainerTypeCatalog.ts`
15. `src/crosshook-native/src/hooks/useRunExecutable.ts`
16. `src/crosshook-native/src/hooks/useSetTrainerVersion.ts`
17. `src/crosshook-native/src/hooks/useGameMetadata.ts`
18. `src/crosshook-native/src/hooks/useInstallGame.ts`
19. `src/crosshook-native/src/hooks/useProtonMigration.ts`
20. `src/crosshook-native/src/hooks/useGameCoverArt.ts` (also uses
    `convertFileSrc` — migrate to the plugin stub).
21. `src/crosshook-native/src/hooks/useLauncherManagement.ts`
22. `src/crosshook-native/src/hooks/useProtonInstalls.ts`
23. `src/crosshook-native/src/hooks/useProtonDbLookup.ts`
24. `src/crosshook-native/src/hooks/useOnboarding.ts`
25. `src/crosshook-native/src/hooks/useProtonUp.ts`
26. `src/crosshook-native/src/hooks/useProfileHealth.ts`
27. `src/crosshook-native/src/hooks/useExternalTrainerSearch.ts`
28. `src/crosshook-native/src/hooks/usePreviewState.ts`
29. `src/crosshook-native/src/hooks/useTrainerDiscovery.ts`
30. `src/crosshook-native/src/hooks/useOfflineReadiness.ts`
31. `src/crosshook-native/src/hooks/useLaunchState.ts`
32. `src/crosshook-native/src/hooks/useUpdateGame.ts`
33. `src/crosshook-native/src/hooks/useMangoHudPresets.ts`
34. `src/crosshook-native/src/hooks/useCommunityProfiles.ts`
35. `src/crosshook-native/src/hooks/useGameDetailsProfile.ts`
36. `src/crosshook-native/src/hooks/useProtonDbSuggestions.ts`
37. `src/crosshook-native/src/hooks/usePrefixDeps.ts`
38. `src/crosshook-native/src/hooks/useProfile.ts` (largest migration —
    dozens of `invoke` sites).
39. `src/crosshook-native/src/hooks/usePrefixStorageManagement.ts`

Components:

40. `src/crosshook-native/src/components/SteamLaunchOptionsPanel.tsx`
41. `src/crosshook-native/src/components/ProfileActions.tsx`
42. `src/crosshook-native/src/components/AutoPopulate.tsx`
43. `src/crosshook-native/src/components/CommunityImportWizardModal.tsx`
44. `src/crosshook-native/src/components/LauncherExport.tsx`
45. `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`
46. `src/crosshook-native/src/components/profile-sections/MediaSection.tsx`
    (also uses `convertFileSrc` — migrate to the plugin stub).
47. `src/crosshook-native/src/components/LaunchPanel.tsx`
48. `src/crosshook-native/src/components/OnboardingWizard.tsx`
49. `src/crosshook-native/src/components/SettingsPanel.tsx`
50. `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`
51. `src/crosshook-native/src/components/pages/LaunchPage.tsx`
52. `src/crosshook-native/src/components/pages/ProfilesPage.tsx`
53. `src/crosshook-native/src/components/layout/ConsoleDrawer.tsx`
54. `src/crosshook-native/src/components/ConsoleView.tsx`
55. `src/crosshook-native/src/components/PrefixDepsPanel.tsx`

The migration is mechanical. A one-shot find/replace pass plus a manual
review of the diff covers every site.

---

## Open Questions

1. **`convertFileSrc` synchrony — eager init vs Vite alias.** Current plan
   is Option 1 (eager `initConverters()` at startup). Option 2 (Vite
   `resolve.alias` for that single export) is cleaner if anyone objects to
   the 3-line change in `main.tsx`. Flagged for reviewer judgment.
2. **Dialog stub UX.** `dialog.open()` currently returns `null` in browser
   mode, which looks like user-cancelled. UX research suggests a visible
   toast instead of silent `console.warn`. Phase 1 ships with the warn;
   Phase 2 or Phase 4 upgrades to a toast. Confirm acceptable.
3. **CI sentinel string list.** The current grep set
   (`registerMocks`, `lib/mocks`, `[mock] callCommand`, `MOCK MODE`) is
   good coverage but not exhaustive. Should we add a dedicated
   `MOCK_FIXTURE_MARKER = 'crosshook-mock-fixture-marker-0001'` constant
   that's the single authoritative sentinel, simpler to grep than four
   strings? Proposed yes.
4. **`__WEB_DEV_MODE__` compile-time constant.** Not required today — the
   runtime `isTauri()` gate plus CI sentinel is sufficient. Adopt only if
   the sentinel ever trips. Alternative `import.meta.env.MODE === 'webdev'`
   is strictly additive if `npm run dev:browser` is retargeted to
   `vite --mode webdev`.
5. **Stateful mocks for round-trip UX.** The store pattern already allows
   mutation. Do we want `localStorage` persistence across page reloads, or
   keep the "reload resets" semantics? Current plan: reload resets, since
   designers value reproducible starting states. Revisit if UX pushback.
6. **Fixture variant toggles (`?onboarding=failed`, `?library=empty`,
   `?errors=true`).** Deferred to Phase 4. Confirm with ux-researcher
   whether MVP acceptance criteria include any of these.
7. **Cover art — CDN vs local.** `public/mock-art/` with placeholder PNGs
   keeps webdev offline. Alternative: reference Steam's public CDN for
   accurate asset dimensions but introduce a network dependency. Current
   plan: local placeholders.
8. **Event payload type extraction.** 16 event channels discovered from
   frontend `listen(...)` calls. Rust-side `emit(...)` payload types
   should ideally be codegen'd into `types/events.ts` to make fixture
   event payloads type-safe. Out of Phase 3 scope; follow-up task.
9. **Plugin module import-time side effects.** `@tauri-apps/plugin-fs`,
   `plugin-dialog`, `plugin-shell` may touch `window.__TAURI_INTERNALS__`
   at module evaluation. If so, the `import { open } from
'@tauri-apps/plugin-dialog'` line throws before our stub runs. Verified
   mitigation: the plugin stub files do their dynamic imports _inside_ the
   `isTauri()` branch, so import-time side effects never fire in browser
   mode.
10. **Handler file granularity.** 23 handler files (1:1 with Rust command
    modules) vs ~5 broader groupings. Plan favors 1:1 because it minimizes
    drift when a new Rust command lands — the same review path adds the
    mock. Confirm with practices-researcher.

---

## Other Docs

- `docs/plans/dev-web-frontend/research-business.md` — User stories, business rules, URL toggle specs, storage boundary, success criteria
- `docs/plans/dev-web-frontend/research-ux.md` — DevModeChip CSS token spec, fixture state switching UX, URL toggle design rationale
- `docs/plans/dev-web-frontend/research-practices.md` — Engineering practices evaluation, KISS assessment, full callsite inventory
- `docs/plans/dev-web-frontend/research-security.md` — Security implications of browser-exposed dev server, CI sentinel details
- `docs/plans/dev-web-frontend/research-recommendations.md` — Cross-team synthesis, phasing plan, alternative approaches comparison table
- `src/crosshook-native/src/context/PreferencesContext.tsx` — boot-sequence triple-invoke pattern (lines 43–47)
- `src/crosshook-native/src/types/settings.ts` — `DEFAULT_APP_SETTINGS` ready-made fixture (line 69)
- `src/crosshook-native/src/types/profile.ts` — `createDefaultProfile()` ready-made profile fixture (line 281)
- `src/crosshook-native/vite.config.ts` — current Vite configuration (to be updated)
- `scripts/dev-native.sh` — current dev script (to be updated with `--dev` branch)
- `src/crosshook-native/src/App.tsx` — `onboarding-check` listen location (line 67); `AppShell` structure
