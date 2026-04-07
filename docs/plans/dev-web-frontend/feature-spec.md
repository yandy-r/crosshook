# Feature Spec: dev-web-frontend

> Browser-only Vite dev mode for CrossHook's React frontend. A new `./scripts/dev-native.sh --browser` flag starts only the Vite dev server at `http://localhost:5173` and routes every Tauri IPC call through a typed, hand-rolled mock adapter, enabling full UI/UX iteration in a real browser with DevTools, logs, element inspection, and deterministic fixture data — without a Rust toolchain, without compiling `crosshook-core`, and without changing a single byte of the production AppImage.

---

## Executive Summary

CrossHook's React frontend calls `invoke()` from `@tauri-apps/api/core` at **84 unique call sites across 35 files**, plus `listen()` at 16 sites, plus three plugin packages (`plugin-dialog`, `plugin-shell`, `plugin-fs`) and `convertFileSrc`. None of these resolve in a plain browser, so `http://localhost:5173` currently renders an empty shell. This feature introduces a thin, owned **IPC adapter layer** under `src/crosshook-native/src/lib/` (`runtime.ts`, `ipc.ts`, `events.ts`, plus `mocks/` and `plugin-stubs/`) that routes to the real Tauri APIs when `isTauri()` is true and to a function-map of deterministic fixtures otherwise. Call sites migrate mechanically (`invoke(` → `callCommand(`, `listen(` → `subscribeEvent(`) and the entire mock subtree is dynamically imported behind a `__WEB_DEV_MODE__` Vite `define` constant so Rollup eliminates it from the production chunk graph — with a CI grep sentinel on `dist/assets/*.js` as the authoritative safety net. **Zero new dependencies**, no Storybook, no MSW; the only adopted library is the mocking capability already bundled in `@tauri-apps/api@^2.0.0` (which we deliberately do not use in favor of the hand-rolled adapter for broader surface coverage). Primary challenges are the mechanical scale of the migration, the maintenance tax of ~18 handler files tracking the Rust IPC contract, and making absolutely certain no mock code ships inside the AppImage.

---

## External Dependencies

### APIs and Services

No external APIs or network services. CrossHook is an offline desktop app; this feature introduces a browser variant of the same offline frontend. No network calls are added.

### Libraries and SDKs

**Zero new dependencies are required.** Every library evaluated during research was rejected for MVP scope.

| Library                             | Version                      | Purpose                                                                                            | Installation                   | Verdict                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------- | ---------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@tauri-apps/api`                   | `^2.0.0`                     | Already present — provides `invoke`, `isTauri`, `listen`, `convertFileSrc` used in Tauri mode path | Already installed              | **Reused** (existing dep)                                                                                                                                                                                                                                                                                                                 |
| `@tauri-apps/api/mocks` (`mockIPC`) | ships with `@tauri-apps/api` | Alternative "drop-in" mock IPC system                                                              | N/A — already on disk          | **Rejected** — covers only `invoke`; does not cover `listen`, plugin packages, or `convertFileSrc`; poisons `__TAURI_INTERNALS__`-based detection probes; designed for Vitest/Jest teardown patterns, not persistent dev sessions                                                                                                         |
| `msw`                               | 2.12.14                      | Industry-standard HTTP mock library                                                                | Would be a new `devDependency` | **Rejected** — wrong interception layer. Tauri `invoke()` is a WebView bridge call, not HTTP. MSW's Service Worker cannot intercept it. Also writes `mockServiceWorker.js` to `public/` via a postinstall hook that would get copied into `dist/`.                                                                                        |
| `@faker-js/faker`                   | 10.4.0                       | Synthetic data generator                                                                           | Would be new                   | **Rejected for MVP** — hand-authored static TypeScript fixtures are reviewable at a glance and eliminate the dependency entirely. `DEFAULT_APP_SETTINGS` and `createDefaultProfile()` already exist as ready-made fixtures. Historical sabotage incident (Jan 2022 `faker.js` event) requires exact-version pinning if ever reconsidered. |
| Storybook 8 / Ladle                 | 8.x / latest                 | Component isolation                                                                                | Would be new                   | **Deferred** — solves a different problem (per-component isolation, not full-app flows). Ladle preferred over Storybook if ever adopted for this React-only stack.                                                                                                                                                                        |

**Verified against `@tauri-apps/api@2.10.1` source**: the initial research claim that `mockIPC` "requires the Tauri bridge" is **incorrect** — `mockIPC` calls `mockInternals()` which does `window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ ?? {}`, so it works in a plain browser. The correct reason to prefer the hand-rolled adapter is surface coverage (it handles events, plugins, and `convertFileSrc` in one place) and the fact that `mockIPC` silently returns `undefined` for unregistered commands whereas the hand-rolled adapter throws loudly by design.

### External Documentation

- [Tauri v2 Mocking Docs](https://v2.tauri.app/develop/tests/mocking/) — reference for the alternative `mockIPC` approach
- [Tauri v2 `isTauri()` discussion #6119](https://github.com/tauri-apps/tauri/discussions/6119) — canonical detection helper
- [Vite: Env Variables and Modes](https://vite.dev/guide/env-and-mode) — `import.meta.env.DEV`, `--mode`, `define` behavior
- [Vite Security Advisory GHSA-vg6x-rcgg-rjx6](https://github.com/vitejs/vite/security/advisories/GHSA-vg6x-rcgg-rjx6) — CORS/WebSocket source theft (patched in Vite ≥6.0.9; CrossHook pins `^8.0.5` so unaffected)
- [Vite Issue #11080](https://github.com/vitejs/vite/issues/11080) — dynamic imports tree-shaking limitation (informs the `__WEB_DEV_MODE__` guard strategy)
- [Chrome DevTools Workspaces](https://developer.chrome.com/docs/devtools/workspaces) — workflow this feature unlocks

---

## Business Requirements

### User Stories

**Primary User: Frontend developer / designer contributing to CrossHook**

- As a frontend developer iterating on CSS, I want to open `http://localhost:5173` in Chrome and see the full CrossHook shell populated with realistic mock data, so that I can use DevTools element inspection, live CSS editing, and Workspaces to iterate in seconds instead of waiting for a Tauri rebuild.
- As a designer, I want to switch between `empty`, `populated`, `error`, and `loading` states by changing a URL query parameter, so that I can verify every route's edge-state UI without touching code or restarting Vite.
- As a developer, I want an unmistakable dev-mode visual indicator that cannot be cropped out of a screenshot, so that I never accidentally mistake a mock session for the real app when sharing screenshots in issues or PRs.

**Secondary User: New contributor without a Rust toolchain**

- As a new contributor who has Node.js but not Rust installed, I want to run `./scripts/dev-native.sh --browser` (or `npm run dev` inside `src/crosshook-native/`) and have the full UI render in my browser, so that I can make CSS or component contributions without setting up WebKitGTK dev libraries or installing `cargo`.

**Tertiary User: Maintainer reviewing a design PR**

- As a maintainer, I want a dev-browser mode that uses the identical React component tree as production, so that I can trust "it looks right in browser mode" translates to "it looks right in Tauri mode" for layout-level changes.

### Business Rules

1. **BR-1 — No mock code in production builds.** The mock IPC adapter, mock fixtures, plugin stubs, and dev-mode indicator must not appear in any bundle produced by `vite build` during a Tauri AppImage build. Enforced by a CI grep sentinel on `dist/assets/*.js` that fails the build if any of `[dev-mock]`, `getMockRegistry`, `registerMocks`, or `MOCK MODE` appear. The `__WEB_DEV_MODE__` Vite `define` constant + dynamic-import gate is the primary mechanism; the CI grep is the authoritative safety net.
   - Validation: CI job `verify:no-mocks` runs after `Build native AppImage` in `.github/workflows/release.yml` and `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` continues to pass.

2. **BR-2 — Mock mode must not require Rust toolchain.** The `--browser` code path must never invoke `cargo`, `tauri`, or any native binary. Only `npm` / `node` / Vite are permitted.
   - Validation: Running the script inside a container without `cargo` installed produces a working dev server.

3. **BR-3 — Same React component tree, no divergent code paths.** The identical components, hooks, and context providers used in production must be used in mock mode. The IPC boundary (`callCommand` / `subscribeEvent` / plugin stubs) is the ONLY swapped layer. No duplicate page or route components. No per-route `if (isBrowser)` branches. All detection is confined to `lib/ipc.ts`, `lib/events.ts`, and the plugin stub modules.

4. **BR-4 — Fail-fast on missing mocks.** `callCommand('foo')` in browser mode where `foo` has no registered handler MUST throw `[dev-mock] Unhandled command: foo. Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md`. Silent `undefined` returns are forbidden because they mask coverage gaps.

5. **BR-5 — All mock state is runtime-only.** No `localStorage`, no `IndexedDB`, no disk writes. In-memory `Map` in `lib/mocks/store.ts` resets on page reload. This is intentional — reproducible starting states are more valuable than cross-session persistence for design iteration.

6. **BR-6 — Mutating commands return the stored object, not void.** `profile_save`, `settings_save`, etc. must update the in-memory store AND return the saved payload, so components that optimistically re-read after write continue to function.

7. **BR-7 — Listen stubs return a real cleanup function.** `subscribeEvent(name, handler)` in browser mode must return a resolved `Promise<() => void>` with a working unsubscribe function, not a no-op. Hooks rely on the unlisten return value in `useEffect` cleanup, and accumulating unresolvable teardown calls would create memory-leak patterns in long-running dev sessions.

8. **BR-8 — Plugin stubs fail loudly on destructive operations.** `plugin-fs.writeFile`, `plugin-shell.execute`, etc. MUST `throw new Error('plugin-fs/writeFile is not available in browser dev mode')` rather than silently no-op. File pickers in `plugin-dialog` may return `null` (mimicking cancellation) but MUST also `console.warn('[dev-mock] dialog suppressed')` and optionally surface a toast, so developers notice that the dialog never appeared.

9. **BR-9 — Vite dev server must bind to loopback only.** `vite.config.ts` must force `server.host = '127.0.0.1'` and `server.strictPort = true` for the `--mode webdev` path. `--host 0.0.0.0` is explicitly unsupported. Documented in `scripts/dev-native.sh` help text and `AGENTS.md`.

10. **BR-10 — Fixture content policy: obviously fake.** Fixture game titles must use placeholder names (`Test Game Alpha`, `Dev Game Beta`), Steam App IDs must be outside the valid range (e.g., `9999001`+), and no path may reference a real user directory. PR review checklist for changes under `lib/mocks/` enforces this.

11. **BR-11 — `?fixture=` is the URL-driven state switcher.** Query-string values `populated` (default), `empty`, `error`, `loading` select the active fixture scenario. Unknown values fall back to `populated`. Shell-critical reads (`settings_load`, `profile_list`) continue to resolve successfully even in `error` state so the app shell can render.

12. **BR-12 — Orthogonal debug toggles: `?errors=true`, `?delay=<ms>`, `?onboarding=show`.** `errors=true` rejects write/action commands but leaves reads unaffected; `delay=N` wraps all responses in a `setTimeout(N)`; `onboarding=show` surfaces the onboarding wizard on mount by synthesizing the `onboarding-check` event the backend never fires in browser mode.

### Edge Cases

| Scenario                                                                           | Expected Behavior                                                                                                                                                                                                                                              | Notes                                                                                                               |
| ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Developer opens `http://localhost:5173` without running the Vite dev server        | Browser shows "This site can't be reached"                                                                                                                                                                                                                     | Normal browser error; nothing to implement.                                                                         |
| Mock handler throws inside `callCommand`                                           | The returned promise rejects with the thrown error; hooks surface it via their existing `try/catch` or error-boundary paths                                                                                                                                    | Matches real Tauri error shape.                                                                                     |
| Component tries to call `convertFileSrc` on a game cover path in browser mode      | Returns the path unchanged (passthrough); the resulting `<img src>` will fail to load and fall back to placeholder                                                                                                                                             | `useGameCoverArt.ts` and `MediaSection.tsx` — no crash.                                                             |
| Vite HMR reloads `lib/mocks/handlers/profile.ts`                                   | The mock registry singleton resets; next `callCommand` rebuilds it; the in-memory store also resets                                                                                                                                                            | Intended behavior; fixture edits are picked up without a manual reload.                                             |
| Developer adds a new `#[tauri::command]` in Rust but forgets to add a mock handler | Next `callCommand` invocation in browser mode throws `[dev-mock] Unhandled command: new_command` with a pointer to `lib/mocks/README.md`                                                                                                                       | Fail-fast surface. A follow-up Phase 3 script `dev:browser:check` can grep Rust sources and list unmocked commands. |
| A developer runs `vite dev` directly (not via `./scripts/dev-native.sh --browser`) | Works identically — `vite dev` auto-sets `MODE=development`; the adapter routes to real Tauri if inside a WebView, otherwise to mocks                                                                                                                          | The script is a convenience, not a gate.                                                                            |
| A developer runs `vite build --mode webdev` and deploys `dist/` to a web host      | **Intentional foot-gun.** Would ship mock code as a public website. Mitigation: the `verify:no-mocks` CI check only runs on Tauri build pipelines; there is no automatic protection for ad-hoc builds. Documented in `lib/mocks/README.md` as "do not deploy". |
| `mockIPC` is accidentally called alongside the hand-rolled adapter                 | `window.__TAURI_INTERNALS__` gets set; any hand-rolled `isTauri()` using the `__TAURI_INTERNALS__` probe would break. **Mitigation**: use the official `isTauri()` from `@tauri-apps/api/core` which checks `globalThis.isTauri` — unaffected by `mockIPC`.    |
| A developer opens the dev server URL on a phone via `--host 0.0.0.0`               | The `strictPort` + `host: '127.0.0.1'` config rejects it. Anyone who explicitly overrides config is assumed to have read the security policy.                                                                                                                  |

### Success Criteria

- [ ] **SC-1** `./scripts/dev-native.sh --browser` starts a Vite dev server and does NOT invoke `cargo`, `tauri`, or any native binary (verified by observing the process tree).
- [ ] **SC-2** `http://localhost:5173` renders all 9 routes (`library`, `profiles`, `launch`, `install`, `community`, `discover`, `compatibility`, `settings`, `health`) without JavaScript errors in the browser console.
- [ ] **SC-3** The two-layer dev-mode indicator (`.crosshook-app--webdev` inset amber outline + `<DevModeChip />` corner chip) is visible on every route in `--browser` mode and ABSENT from Tauri-mode (`./scripts/dev-native.sh` without a flag).
- [ ] **SC-4** Production AppImage build (`./scripts/build-native.sh`) produces a bundle that does NOT contain any of `[dev-mock]`, `getMockRegistry`, `registerMocks`, or `MOCK MODE` in `dist/assets/*.js` — verified by the `verify:no-mocks` CI grep in `release.yml`.
- [ ] **SC-5** HMR latency: editing `variables.css` reflects in the browser within Vite's normal sub-2s window.
- [ ] **SC-6** `listen()` and its migrated `subscribeEvent()` counterparts do not throw or crash when events never fire (empty in-process bus). After 5 seconds of idle, the browser console is clean.
- [ ] **SC-7** File picker stubs (`plugin-dialog.open`, `.save`) return `null` without crashing and emit `console.warn('[dev-mock] dialog suppressed')`.
- [ ] **SC-8** `convertFileSrc`-dependent cover art cells in Library render a placeholder image without throwing; no "broken image" icons for cells that would otherwise reference `tauri://` asset URLs.
- [ ] **SC-9** `?fixture=empty` renders all list views empty (Library, Profiles, Community, Discover, Health) with no JS errors.
- [ ] **SC-10** `?fixture=error` rejects fallible commands and surfaces their error UI; the app shell continues to render.
- [ ] **SC-11** `?fixture=loading` shows skeleton/spinner states for all async views.
- [ ] **SC-12** Dev-mode chip label updates to match the active fixture state (`DEV · populated`, `DEV · empty`, etc.).
- [ ] **SC-13** `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` continues to pass after the migration (no Rust changes are expected, but the TS migration is large enough to warrant verification).
- [ ] **SC-14** The mechanical migration (`invoke(` → `callCommand(`, `listen(` → `subscribeEvent(`, plugin import rewrites) touches all 84 + 16 + N sites consistently — no stragglers verified by `rg 'from [\'"]@tauri-apps/api/core[\'"]'` producing zero hits outside `lib/ipc.ts`, `lib/runtime.ts`, and the Tauri branch of `lib/plugin-stubs/convertFileSrc.ts`.

---

## Technical Specifications

### Architecture Overview

```text
┌──────────────────── Browser or Tauri WebView ────────────────────┐
│                                                                  │
│  React Components (unchanged)                                    │
│  ├─ Pages: Library, Profiles, Launch, Install, Community, ...    │
│  ├─ Hooks: useProfile, useLaunchState, useProfileHealth, ...     │
│  └─ Contexts: ProfileContext, PreferencesContext, ...            │
│                                                                  │
│         ↓ imports `callCommand`, `subscribeEvent`, plugin stubs  │
│                                                                  │
│  ┌────────────── lib/ (new adapter layer) ──────────────────┐    │
│  │                                                          │    │
│  │  runtime.ts   → isTauri() probe (single source)          │    │
│  │  ipc.ts       → callCommand<T> adapter                   │    │
│  │  events.ts    → subscribeEvent<T> + emitMockEvent        │    │
│  │  plugin-stubs/ → dialog.ts, shell.ts, fs.ts,             │    │
│  │                  convertFileSrc.ts                       │    │
│  │                                                          │    │
│  │  ── branch on isTauri() at runtime ──                    │    │
│  │      ↓ true                    ↓ false                   │    │
│  └──────┼────────────────────────┼──────────────────────────┘    │
│         ↓                        ↓                               │
│   Real Tauri APIs        lib/mocks/ (DEV-only)                   │
│   @tauri-apps/api/core   ├─ index.ts (registerMocks)             │
│   @tauri-apps/api/event  ├─ store.ts (in-memory MockStore)       │
│   @tauri-apps/plugin-*   ├─ eventBus.ts (in-process pub/sub)     │
│                          └─ handlers/                            │
│                             ├─ settings.ts                       │
│                             ├─ profile.ts                        │
│                             ├─ launch.ts                         │
│                             ├─ health.ts                         │
│                             ├─ install.ts  (~18 files total)     │
│                             └─ ...                               │
│                                                                  │
│  ┌── __WEB_DEV_MODE__ Vite `define` constant ──┐                 │
│  │  true  in `vite --mode webdev`              │                 │
│  │  false in `vite build` (tauri production)  │                  │
│  │  → Rollup eliminates entire lib/mocks/     │                  │
│  │    chunk graph at build time                │                 │
│  └─────────────────────────────────────────────┘                 │
└──────────────────────────────────────────────────────────────────┘

         ↓ production build pipeline safety net

┌─────────── CI verify:no-mocks (release.yml) ───────────┐
│  grep -rl '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' │
│    src/crosshook-native/dist/assets/*.js                │
│  → fail if any hit                                      │
└─────────────────────────────────────────────────────────┘
```

**Key design decisions** (cross-referenced from `research-technical.md`, `research-practices.md`, `research-security.md`):

- **Single owned boundary**: one adapter layer covers `invoke`, `listen`, three plugin packages, and `convertFileSrc`. No scattered detection. No per-component `if (isBrowser)` checks.
- **Runtime routing via `isTauri()`**, **build-time exclusion via `__WEB_DEV_MODE__`**: `isTauri()` handles _which branch to execute_ at runtime; the `define` constant handles _whether the mock branch exists at all_ in the build. Both are necessary — neither alone is sufficient for production safety.
- **Dynamic `import('./mocks')` inside the `!__WEB_DEV_MODE__` dead branch**: when `__WEB_DEV_MODE__ = false` in production, Rollup sees `if (false) { import('./mocks') }` and drops the dynamic import from the chunk graph before minification.
- **CI grep sentinel is the authoritative control**: even with the `define` guard, Rollup's dynamic-import handling has known edge cases (Vite #11080). The grep is the fail-safe.
- **Mechanical migration, not codemod magic**: 84 `invoke(` → `callCommand(` replacements and 16 `listen(` → `subscribeEvent(` replacements are pure find/replace. Every call site gains the benefit of a visible, owned boundary.

### Data Models

**This feature introduces no new persistent data models.** Per the repo's persistence classification (CLAUDE.md → "Storage boundary"), every piece of state is **runtime-only**:

| Data                          | Classification           | Reason                                                                                |
| ----------------------------- | ------------------------ | ------------------------------------------------------------------------------------- |
| Mock profile list, fixtures   | Runtime-only (ephemeral) | Static TS constants; discarded on tab/Vite restart                                    |
| Mock settings values          | Runtime-only (ephemeral) | In-memory defaults from `DEFAULT_APP_SETTINGS`; never written to `settings.toml`      |
| Dev-mode indicator visibility | Runtime-only (ephemeral) | Determined by `__WEB_DEV_MODE__` compile-time constant                                |
| Active fixture state          | Runtime-only (ephemeral) | Read from `window.location.search` on init                                            |
| Mock event bus state          | Runtime-only (ephemeral) | Held in a module-level `Map<string, Set<Listener>>` in `lib/events.ts`; resets on HMR |

- **No new TOML settings keys**
- **No new SQLite metadata tables, columns, or migrations**
- **No new files on disk**
- **No migrations, no backward-compat shims, no offline/degraded-mode matrix**
- The persistence-plan subsection required by CLAUDE.md is: _"This feature adds no persisted data. It is additive to the dev workflow only, and the production Tauri build is byte-identical whether or not the feature is present, enforced by CI sentinel grep on the AppImage bundle."_

#### Fixture Type Inventory (reused from existing `types/`)

Mock handlers consume existing TypeScript types — zero type duplication.

| Type                                              | Source                                                                                                           | Used by mock handlers                                                 |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `AppSettings` / `AppSettingsData`                 | `types/settings.ts` (`DEFAULT_APP_SETTINGS`)                                                                     | `settings_load`, `settings_save`                                      |
| `Profile` / `SerializedGameProfile`               | `types/profile.ts` (`createDefaultProfile()`, `normalizeSerializedGameProfile`)                                  | `profile_load`, `profile_save`, `profile_duplicate`, `profile_rename` |
| `ProfileSummary`                                  | `hooks/useLibrarySummaries.ts` — **not currently exported**, must be moved to `types/library.ts` as Phase 1 task | `profile_list_summaries`, `profile_list_favorites`                    |
| `RecentFilesData`                                 | `types/settings.ts`                                                                                              | `recent_files_load`, `recent_files_save`                              |
| `LaunchResult`, launch events                     | `types/launch.ts`                                                                                                | `launch_game`, `launch-log` event-bus payloads                        |
| `InstallStatus`, install events                   | `types/install.ts`                                                                                               | `install_game`, `install-*` event-bus payloads                        |
| `OptimizationCatalogPayload`                      | `utils/optimization-catalog.ts`                                                                                  | `get_optimization_catalog`                                            |
| `EnrichedHealthSummary`, `CachedHealthSnapshot[]` | `types/health.ts`                                                                                                | `batch_validate_profiles`, `get_cached_health_snapshots`              |
| `ProtonInstallOption[]`                           | `types/proton.ts`                                                                                                | `list_proton_installs`                                                |
| `ReadinessCheckResult`                            | `types/onboarding.ts`                                                                                            | `check_readiness`                                                     |

#### Mutable In-Memory Store

```ts
// src/crosshook-native/src/lib/mocks/store.ts
type MockStore = {
  settings: AppSettings;
  recentFiles: RecentFilesData;
  profiles: Map<string, Profile>; // keyed by profile name/id
  activeProfileId: string | null;
  prefixPaths: Record<string, string>;
  // ... per-handler-domain state as needed
};
```

The store is initialized once per page load from exported default fixtures, mutated by save/rename/duplicate handlers, and read by list/load handlers. Reloading the tab (Ctrl+R) resets it.

### API Design

**No new HTTP/REST endpoints.** This feature operates entirely at the Tauri IPC boundary, which is already the application's "API" surface.

#### New TypeScript Adapter Surface

##### `lib/runtime.ts` — isTauri() probe

```ts
// src/crosshook-native/src/lib/runtime.ts
export function isTauri(): boolean {
  // globalThis.isTauri is set by Tauri v2 WebView bridge.
  // Unaffected by @tauri-apps/api/mocks.mockIPC (which sets __TAURI_INTERNALS__).
  return !!(globalThis as unknown as Record<string, unknown>).isTauri;
}
```

- Zero dependencies, no React, no DOM required
- Testable in Node with or without JSDOM
- Single source of truth — every other module imports from here

##### `lib/ipc.ts` — callCommand<T> adapter

```ts
// src/crosshook-native/src/lib/ipc.ts
import type { InvokeArgs } from '@tauri-apps/api/core';
import { isTauri } from './runtime';

declare const __WEB_DEV_MODE__: boolean;

type Handler = (args: unknown) => unknown | Promise<unknown>;
let mockMap: Map<string, Handler> | null = null;

async function ensureMocks(): Promise<Map<string, Handler>> {
  if (mockMap) return mockMap;
  if (!__WEB_DEV_MODE__) {
    throw new Error('[dev-mock] mock layer invoked in non-webdev build');
  }
  const { registerMocks } = await import(/* @vite-ignore */ './mocks');
  mockMap = registerMocks();
  return mockMap;
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
      `[dev-mock] Unhandled command: ${name}. Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md`
    );
  }
  if (import.meta.env.DEV) {
    console.debug('[mock] callCommand', name, args);
  }
  return handler(args ?? {}) as Promise<T>;
}
```

##### `lib/events.ts` — subscribeEvent<T> adapter

```ts
// src/crosshook-native/src/lib/events.ts
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

##### `lib/mocks/index.ts` — mock registry

```ts
// src/crosshook-native/src/lib/mocks/index.ts
import { registerSettings } from './handlers/settings';
import { registerProfile } from './handlers/profile';
// ... other handler imports (added in Phase 2)

type Handler = (args: unknown) => unknown | Promise<unknown>;

export function registerMocks(): Map<string, Handler> {
  const map = new Map<string, Handler>();
  registerSettings(map);
  registerProfile(map);
  // ... registerLaunch(map), registerHealth(map), etc.
  return map;
}
```

Each domain handler file exports a `register(map)` function that populates the central map with its command-to-handler entries. Domain handler files created on-demand in Phase 2 — not all upfront.

##### Example Handler: `lib/mocks/handlers/settings.ts`

```ts
// src/crosshook-native/src/lib/mocks/handlers/settings.ts
import { DEFAULT_APP_SETTINGS } from '../../../types/settings';
import type { AppSettings } from '../../../types/settings';
import { getStore } from '../store';

type Handler = (args: unknown) => unknown | Promise<unknown>;

export function registerSettings(map: Map<string, Handler>): void {
  map.set('settings_load', async () => getStore().settings);
  map.set('settings_save', async (args) => {
    const next = (args as { settings: AppSettings }).settings;
    getStore().settings = next;
    return next;
  });
  map.set('recent_files_load', async () => getStore().recentFiles);
  map.set('recent_files_save', async (args) => {
    getStore().recentFiles = (args as { recent: typeof DEFAULT_APP_SETTINGS }).recent as any;
    return getStore().recentFiles;
  });
  map.set('default_steam_client_install_path', async () => '/home/devuser/.steam/steam');
}
```

**Errors:** `callCommand` rejects with `Error: [dev-mock] Unhandled command: <name>` for unregistered commands. Handlers may throw any error they choose; the rejected promise propagates through the existing React error-handling paths. Error fixture state is implemented by having selected handlers throw synthetic errors.

### System Integration

#### Files to Create

**Phase 1 (Foundation — single PR):**

- `src/crosshook-native/src/lib/runtime.ts` — `isTauri()` probe
- `src/crosshook-native/src/lib/ipc.ts` — `callCommand<T>` adapter
- `src/crosshook-native/src/lib/events.ts` — `subscribeEvent<T>` adapter + `emitMockEvent` + in-process `browserBus`
- `src/crosshook-native/src/lib/DevModeBanner.tsx` — Layer 2 corner chip component (`<DevModeChip />` naming is acceptable; see UX research for the Layer 1 inset outline CSS)
- `src/crosshook-native/src/lib/dev-indicator.css` — `.crosshook-app--webdev { box-shadow: inset 0 0 0 3px var(--crosshook-color-warning); }` and corner chip sizing overrides; imported only in the `__WEB_DEV_MODE__` branch so it never enters the production stylesheet
- `src/crosshook-native/src/lib/plugin-stubs/dialog.ts` — real plugin in Tauri; `console.warn` + `null` in browser; toast on destructive ops
- `src/crosshook-native/src/lib/plugin-stubs/shell.ts` — same pattern; `shell.execute` throws in browser
- `src/crosshook-native/src/lib/plugin-stubs/fs.ts` — same pattern; `writeFile` throws, `readFile` returns stub data
- `src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts` — real `convertFileSrc` in Tauri; passthrough `(path) => path` in browser (first `initConverters()` call is synchronous-safe, see edge case in research-technical.md)
- `src/crosshook-native/src/lib/mocks/index.ts` — `registerMocks()` orchestrator
- `src/crosshook-native/src/lib/mocks/store.ts` — `getStore()` + `MockStore` type
- `src/crosshook-native/src/lib/mocks/eventBus.ts` — glue between handlers and `lib/events.ts`
- `src/crosshook-native/src/lib/mocks/README.md` — contributor guide for adding handlers
- `src/crosshook-native/src/lib/mocks/handlers/settings.ts` — boot-critical handlers
- `src/crosshook-native/src/lib/mocks/handlers/profile.ts` — boot-critical handlers

**Phase 2 (fan-out, per-domain PRs):** one handler file per domain in `src/crosshook-native/src/lib/mocks/handlers/`: `launch.ts`, `health.ts`, `install.ts`, `update.ts`, `proton.ts`, `protondb.ts`, `protonup.ts`, `community.ts`, `discovery.ts`, `launcher.ts`, `prefix.ts`, `runExec.ts`, `diagnostics.ts`, `onboarding.ts`, `catalog.ts`, `art.ts`, `steam.ts`. Approximately **~18 handler files** total (roughly matching the IPC command groupings enumerated in `research-business.md` "IPC Call Inventory").

#### Files to Modify

**Phase 1:**

- `scripts/dev-native.sh` — add `--browser` (`--web` alias) branch; 3–5 lines; no `cargo`/`tauri` invocation in this branch
- `src/crosshook-native/package.json` — add `"dev:browser": "vite --mode webdev"` script
- `src/crosshook-native/vite.config.ts` — add mode-conditional `define: { __WEB_DEV_MODE__: mode === 'webdev' }`; explicit `server.host = '127.0.0.1'` + `server.strictPort = true`; document comment
- `src/crosshook-native/src/vite-env.d.ts` — `declare const __WEB_DEV_MODE__: boolean;`
- `src/crosshook-native/src/App.tsx` — add `__WEB_DEV_MODE__` conditional className on `.crosshook-app` root and render `<DevModeChip />` before `<ProfileProvider>` so it participates in no context
- `src/crosshook-native/src/main.tsx` — no changes expected; mock registration is lazy on first `callCommand`
- `src/crosshook-native/src/types/library.ts` — export `ProfileSummary` type (currently local to `hooks/useLibrarySummaries.ts`)
- **All 35+ files containing `invoke(`** — mechanical `import { invoke } from '@tauri-apps/api/core'` → `import { callCommand } from '@/lib/ipc'` and `invoke(` → `callCommand(`
- **All 13 files containing `listen(` from `@tauri-apps/api/event`** — mechanical `import { listen }` → `import { subscribeEvent } from '@/lib/events'` and `listen(` → `subscribeEvent(`
- **All files importing `@tauri-apps/plugin-dialog`, `-plugin-shell`, `-plugin-fs`** — rewrite import path to the corresponding `lib/plugin-stubs/` module
- `src/crosshook-native/src/utils/optimization-catalog.ts` — single non-hook `invoke` call that must also migrate
- `src/crosshook-native/src/context/PreferencesContext.tsx` — most critical single file: parallel 3-command boot at lines 44–46
- `.github/workflows/release.yml` — add `verify:no-mocks` step after the AppImage build:

  ```yaml
  - name: Verify no mock code in production bundle
    run: |
      if grep -rl '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' \
          src/crosshook-native/dist/assets/*.js 2>/dev/null; then
        echo "ERROR: Mock code found in production bundle" >&2
        exit 1
      fi
      echo "Bundle clean — no mock strings found"
  ```

- `AGENTS.md` — add `./scripts/dev-native.sh --browser` to the Commands short-reference block; note loopback-only binding policy
- `CLAUDE.md` — no changes expected; existing rules cover this

#### Configuration

**`vite.config.ts` additions (mode-conditional):**

```ts
export default defineConfig(({ mode }) => ({
  // ... existing config
  define: {
    __WEB_DEV_MODE__: mode === 'webdev',
  },
  server: {
    // Existing: host: host || false
    // New: strict loopback when mode=webdev
    host: mode === 'webdev' ? '127.0.0.1' : host || false,
    strictPort: mode === 'webdev' ? true : undefined,
    // ...
  },
}));
```

**`package.json` additions:**

```json
{
  "scripts": {
    "dev:browser": "vite --mode webdev"
  }
}
```

**`scripts/dev-native.sh` branch:**

```bash
case "${1:-}" in
  --browser|--web)
    cd "$NATIVE_DIR"
    [[ -x "$NATIVE_DIR/node_modules/.bin/vite" ]] || npm ci
    echo "Starting CrossHook frontend-only dev server (browser mock IPC)..."
    echo "  -> http://localhost:5173"
    exec npm run dev:browser
    ;;
  --help|-h)
    usage
    exit 0
    ;;
  "")
    # existing Tauri dev path
    ;;
  *)
    echo "Error: unknown argument: $1" >&2
    usage >&2
    exit 1
    ;;
esac
```

**CRITICAL: `dev:browser` MUST pass `--mode webdev` explicitly.** Plain `vite` sets `mode = 'development'`, which makes `__WEB_DEV_MODE__ = false`, which silently disables mock activation, which makes `callCommand` try to dynamically import `@tauri-apps/api/core` with no Tauri bridge present, which throws confusingly. This is a documented footgun in `research-security.md` A-1.

---

## UX Considerations

### User Workflows

#### Primary Workflow: Design iteration loop

1. **Start dev mode**
   - User: `./scripts/dev-native.sh --browser`
   - System: Vite starts on port 5173 (loopback only); opens `http://localhost:5173`; the two-layer dev-mode indicator (inset amber viewport outline + corner chip labeled `DEV · populated`) appears on first paint.

2. **Populated fixtures load**
   - User: waits <500ms for boot handlers to resolve
   - System: `PreferencesContext` calls `settings_load`, `recent_files_load`, `default_steam_client_install_path` in parallel; `ProfileContext` calls `profile_list_summaries`; all return from mock handlers in a microtask flush. The UI is fully navigable.

3. **Open DevTools**
   - User: Ctrl+Shift+I, opens Chrome/Firefox DevTools
   - System: Full Chrome DevTools toolkit is available — Element picker (Ctrl+Shift+C), live CSS editing, Workspaces file mapping, Local Overrides, Responsive device emulation, Performance panel. This is the primary value of the feature — unavailable in WebKitGTK.

4. **Edit a component or CSS token**
   - User: edits `variables.css` or a React component file
   - System: Vite HMR applies the change. CSS changes reflect in <200ms without a full reload. React Fast Refresh preserves component state. The mock singleton and in-memory store reset on mock-file HMR only; fixture state re-seeds deterministically from the default store initializer.

5. **Switch fixture state**
   - User: appends `?fixture=empty` (or `?fixture=error`, `?fixture=loading`) to the URL
   - System: full page reload (intentional); the fixture resolver reads `window.location.search` at module-init time; all list views render their empty state; corner chip updates to `DEV · empty`.

6. **Verify WebKitGTK parity**
   - User: periodically re-runs `./scripts/dev-native.sh` (without `--browser`) to verify the change in the real Tauri WebView
   - System: catches the handful of Chrome-vs-WebKit differences documented in `research-ux.md` (scrollbars, `color-mix()`, font rendering, `useScrollEnhance` selector coverage).

#### Error Recovery Workflow

1. **Error occurs**: Developer clicks a "Save Profile" button in browser mode with `?errors=true`.
2. **User sees**: Realistic error toast from the existing error UI path; corner chip shows `DEV · populated · errors`.
3. **Recovery**: Developer iterates on error copy/color, then removes `?errors=true` to verify success path.

#### Missing-Handler Recovery Workflow

1. **Error occurs**: Developer navigates to a page whose IPC command has no registered mock handler.
2. **User sees**: `Error: [dev-mock] Unhandled command: foo_command. Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md` in browser console and React error boundary.
3. **Recovery**: Developer opens `lib/mocks/README.md`, copies the template, adds `map.set('foo_command', async () => MOCK_FOO_RESPONSE)`, saves — HMR picks it up, error disappears.

### UI Patterns

| Component                                   | Pattern                                                                                                                                                                                                                                                                                                        | Notes                                                                                                                                                                                                                                                                                                                                                       |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Layer 1 — Inset viewport outline**        | `box-shadow: inset 0 0 0 3px var(--crosshook-color-warning)` on `.crosshook-app` via `.crosshook-app--webdev` modifier                                                                                                                                                                                         | Zero layout impact (uses `inset` box-shadow, not border); visible on all four edges; survives screenshot crops; cannot be dismissed.                                                                                                                                                                                                                        |
| **Layer 2 — Corner chip `<DevModeChip />`** | `position: fixed; bottom: 12px; right: 12px; z-index: 9999`; reuses `crosshook-status-chip crosshook-status-chip--warning` classes (existing theme tokens); adds `crosshook-dev-chip` for compact sizing (`min-height: 32px; padding: 0 10px; font-size: 0.78rem`) matching `.crosshook-offline-badge` pattern | No close button (non-dismissable per A-6); `role="status"`, `aria-label="Developer mode active. Fixture: {fixture}"`; WCAG AA 8.5:1 contrast confirmed; does NOT use autosave-warning tokens — use `--warning` tokens. Chip renders in `App.tsx` outside `<ProfileProvider>` so it appears before any async fixture loading and survives provider failures. |
| **Fixture switcher**                        | URL query string `?fixture={populated\|empty\|error\|loading}`                                                                                                                                                                                                                                                 | No floating panel; no in-app UI. Full reload on change. Intentional simplicity — design iteration is the target, not fixture QA tooling.                                                                                                                                                                                                                    |
| **Debug toggles**                           | Orthogonal query params `?errors=true`, `?delay=<ms>`, `?onboarding=show`                                                                                                                                                                                                                                      | Combinable (`?fixture=populated&errors=true&delay=800`). Resolved at module init from `window.location.search`.                                                                                                                                                                                                                                             |
| **Missing-mock error**                      | React error boundary surfaces a descriptive error message with a pointer to `lib/mocks/README.md`                                                                                                                                                                                                              | Fail-fast by design — no silent fallbacks.                                                                                                                                                                                                                                                                                                                  |

### Accessibility Requirements

- **WCAG AA contrast**: corner chip text meets 8.5:1 against its background (verified against `--crosshook-color-warning` token)
- **`role="status"`** on the chip so screen readers announce "Developer mode active. Fixture: populated"
- **`aria-live="polite"`** implicit via `role="status"`
- **Keyboard focus not trapped**: the indicator is non-interactive; no `tabindex`, no focusable children
- **Respects `prefers-reduced-motion`**: no animations on the indicator layers
- **Does not break automated a11y audits**: dev-mode indicator must pass axe-core checks even in browser mode so contributors can trust audit results

### Performance UX

- **Loading states**: mock handlers resolve via `Promise.resolve(fixture)` (microtask flush). Spinners and skeletons flash briefly; for a realistic 500 ms loading state, use `?delay=500` or `?fixture=loading` (which never resolves).
- **Optimistic updates**: write handlers (`profile_save`, `settings_save`) mutate the in-memory store synchronously and return the saved payload. Optimistic-UI components feel instant, matching Tauri mode.
- **Error feedback timing**: `?fixture=error` rejects fallible promises; toast timing, error-boundary recovery, and retry affordances are fully testable.
- **HMR + state reset**: fixture file edits reset the mock singleton AND the in-memory store; CSS-only edits don't; component edits preserve React state where possible.
- **Perceived vs real performance**: browser dev mode is a design tool, not a performance profiler. Any latency, frame-rate, or memory analysis must happen against a real Tauri build. Documented in `lib/mocks/README.md`.

---

## Recommendations

### Implementation Approach

**Recommended Strategy:** Hand-rolled `lib/ipc.ts` adapter + per-domain mock handler modules + plugin stubs + two-layer dev-mode indicator + CI grep sentinel.

The adapter pattern is favored over `@tauri-apps/api/mocks` (`mockIPC`) because it covers more surface area in one owned boundary (`invoke`, `listen`, three plugin packages, `convertFileSrc`), fails loudly on missing handlers instead of silently returning `undefined`, and does not depend on a Tauri sub-export whose stability we'd have to track. The migration is mechanical — 84 + 16 find/replace changes — which is tolerable for a one-time cost and actively beneficial for long-term code health (centralized error handling, single grep target, observable IPC surface).

**Phasing:**

1. **Phase 1 — Foundation (one PR, ~50 file changes):** `--browser` script flag, `vite.config.ts` hardening, `lib/` skeleton (`runtime.ts`, `ipc.ts`, `events.ts`, `DevModeChip`, `dev-indicator.css`), all four plugin stubs, mock registry skeleton, `lib/mocks/handlers/settings.ts` + `handlers/profile.ts` (boot-critical), **mechanical migration of all 84 + 16 + N call sites**, export `ProfileSummary` from `types/library.ts`, CI `verify:no-mocks` step, `AGENTS.md` docs update. Validated by `cargo test` + manual smoke test through all 9 routes in both Tauri and browser modes.

2. **Phase 2 — Handler fan-out (~13 sub-PRs, ordered by iteration value):** Profiles → Launch (with `launch-log` event bus) → Health → Onboarding → Install → Update → Proton → ProtonDB → ProtonUp → Community → Launcher → Discovery → everything else. Each sub-PR adds one domain handler file plus its event emitters. `profiles-changed` event broadcast wired into any profile-mutating handler.

3. **Phase 3 — Polish:** `?fixture=empty|error|loading` variants, `?delay=`, `?errors=true`, `?onboarding=show` orthogonal toggles, `dev:browser:check` handler-coverage script, project README documentation, optional Playwright smoke test + visual regression baseline, follow-up `refactor:` issue to move the 13 components that call `invoke()` directly into hooks.

### Technology Decisions

| Decision                | Recommendation                                                                                                               | Rationale                                                                                                                                                                                       |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mock IPC strategy       | **Hand-rolled `callCommand` adapter**                                                                                        | Covers `invoke` + `listen` + plugins + `convertFileSrc` in one owned boundary; loud on missing handlers; no dep on Tauri sub-exports                                                            |
| Mock registry shape     | **Plain `Map<string, Handler>` merged by `registerMocks()`**                                                                 | No DSL, no factory until Rule of Three triggers                                                                                                                                                 |
| Runtime detection       | **`isTauri()` from `@tauri-apps/api/core` (`globalThis.isTauri`)** OR a standalone `lib/runtime.ts` that checks the same key | Both resist `mockIPC`-poisoning. Standalone is explicit; library version avoids circular imports. Either works.                                                                                 |
| Build-time guard        | **`__WEB_DEV_MODE__` via `vite.config.ts` `define` (mode === 'webdev')**                                                     | Stronger Rollup dead-code elimination than `import.meta.env.MODE` string comparison; pairs with CI grep as defense in depth                                                                     |
| Dev server binding      | **Force `server.host = '127.0.0.1'` + `strictPort = true` in webdev mode**                                                   | LAN exposure is a CRITICAL risk; no ad-hoc `--host 0.0.0.0`                                                                                                                                     |
| Dev mode indicator      | **Two layers: inset amber outline + corner chip, no dismiss**                                                                | Non-dismissable per A-6; zero layout impact; survives screenshot crops                                                                                                                          |
| Fixture data source     | **Hand-authored static TS objects reusing `types/*` + `DEFAULT_APP_SETTINGS` + `createDefaultProfile()`**                    | No `faker` dep; reviewable at a glance; TS compiler checks type drift against Rust-shaped returns                                                                                               |
| Fixture state switching | **URL query string `?fixture=<name>` with full reload**                                                                      | Zero UI complexity; URL is shareable; full reload guarantees clean store                                                                                                                        |
| File layout             | **`src/crosshook-native/src/lib/`**                                                                                          | Aligns with existing convention; sits alongside `hooks/`, `components/`, `types/`; NOT `src/platform/web/fixtures/` (empty directory, does not establish convention per research-business AD-2) |
| CI safety gate          | **`verify:no-mocks` grep sentinel in `release.yml`** after AppImage build                                                    | Authoritative control; the define-guard is primary, grep is the fail-safe                                                                                                                       |
| New dependencies        | **Zero**                                                                                                                     | Every library evaluated was rejected for scope, correctness, or surface mismatch                                                                                                                |

### Quick Wins

- **Reuse `DEFAULT_APP_SETTINGS` and `createDefaultProfile()` directly** — no new fixture authoring needed for boot-critical handlers.
- **`console.debug('[mock] callCommand', name, args)` in every browser-mode call** — invaluable for contributors tracing IPC flow; stripped from production via dead-code branch.
- **Mock registry fails loudly** — the moment a contributor opens a page with a missing handler, the error is self-explanatory with a pointer to `lib/mocks/README.md`.
- **Plugin stubs `console.warn` loudly** — no silent no-ops; every suppressed dialog or shell execute is visible in the console.
- **Browser-mode side benefit: contributors get free `useScrollEnhance` sanity checks** — the existing WebKitGTK scroll workaround targets specific selectors; running in Chrome exposes missed selectors immediately.
- **Documentation by example** — `lib/mocks/README.md` with one worked handler copy-paste template keeps the marginal cost of adding new handlers at ~5 minutes.

### Future Enhancements

- **Visual regression baseline** (Phase 3+): Once browser mode exists, Playwright can screenshot every route without a Tauri runtime, unlocking a visual-regression suite currently impossible.
- **Contract test against `#[tauri::command]` registry**: Generate the canonical command list at build time from the Rust side, diff against `registerMocks()` keys, fail CI on drift. High-value medium-effort follow-up once handler coverage stabilizes.
- **Storybook-lite at `/components`** (deferred): Only if contributors actively ask. Solves per-component isolation which is a different problem from full-app iteration.
- **Fix 13 `invoke()`-from-components sites** (tracked as `refactor:` follow-up): Mild architectural smell — business logic in components. Browser-mode migration does the mechanical rewrite; fixing the architectural issue is out of scope.
- **Export `ProfileSummary` from `types/library.ts`** (Phase 1 prerequisite): Small cleanup; unblocks typed mock handlers.
- **localStorage-backed mock store** (opt-in, future): Persist mock state across page reloads for multi-session flows. Only add if anyone asks. Must not introduce PII or security drift; cross-reference security research before implementing.
- **Mock IPC introspection panel** (Phase 4+): Floating dev panel showing every `callCommand` with request/response in real time. Useful but not MVP.

---

## Risk Assessment

### Technical Risks

| Risk                                                               | Likelihood                | Impact       | Mitigation                                                                                                                                                                                                                                                                  |
| ------------------------------------------------------------------ | ------------------------- | ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mock code leaks into production AppImage                           | **Medium**                | **CRITICAL** | `__WEB_DEV_MODE__` Vite `define` + dynamic `import()` in dead branch + CI `verify:no-mocks` grep on `dist/assets/*.js` (sentinel strings: `[dev-mock]`, `getMockRegistry`, `registerMocks`, `MOCK MODE`). Three-layer defense.                                              |
| Mock / Rust IPC contract drift                                     | High                      | High         | Handlers import return types from `src/crosshook-native/src/types/*.ts`; TS compiler catches structural drift. Long-term: generate canonical command list from Rust in CI and diff against `registerMocks()` keys.                                                          |
| Events not mocked in Phase 1 break launch/update/health flows      | High                      | High         | `lib/events.ts` + in-process `browserBus` + `emitMockEvent` is **MVP, not optional**. Phase 2 handlers for launch/update/health MUST call `emitMockEvent` to drive realistic state transitions.                                                                             |
| Plugin stub silent-null bug hides real dialog-dependent bugs       | Medium                    | High         | Dialog stubs `console.warn('[dev-mock] dialog suppressed')` + optional toast; destructive operations (`shell.execute`, `fs.writeFile`) `throw` rather than no-op.                                                                                                           |
| CSP and WebKitGTK behavior differ from Chrome/Firefox              | High                      | Medium       | Document in `lib/mocks/README.md`; corner chip copy reminds "re-verify in `./scripts/dev-native.sh` before merge"; WebKit-only bugs found in production remain in scope for Phase 3.                                                                                        |
| `dev:browser` script omits `--mode webdev` flag                    | Low                       | High         | The npm script `"dev:browser": "vite --mode webdev"` hard-codes the flag. If contributors invoke `vite` directly they are responsible for the flag. Document in `AGENTS.md`.                                                                                                |
| 84-call-site / 18-handler-file maintenance tax                     | **Certain**               | Medium       | Informative unhandled-command error with README pointer; Phase 3 `dev:browser:check` script scans Rust commands and lists unmocked ones.                                                                                                                                    |
| Vite dev server bound to LAN on `--host` override                  | Low (default-safe)        | High         | `server.host = '127.0.0.1'` + `strictPort = true` hardcoded in webdev config; document `--host 0.0.0.0` as unsupported.                                                                                                                                                     |
| Fixture data leaks PII (real game names, real Steam paths)         | Medium                    | Medium       | Mandatory policy in `lib/mocks/README.md` + PR review checklist; fixture constants use obviously-synthetic prefixes (`MOCK_*`, `Test Game Alpha`, Steam IDs ≥ `9999001`). Phase 3+: CI grep for SteamID64 (`\b[0-9]{17}\b`) and home paths scoped to `lib/mocks/fixtures/`. |
| Banner layout intrusion (fixed header shifts `100vh` measurements) | Low (mitigated by design) | Medium       | Two-layer indicator uses `inset box-shadow` (zero layout) and `position: fixed` corner chip (z-index only). No layout shift.                                                                                                                                                |
| First-render flicker from async mock import                        | Low                       | Low          | `callCommand` is async-by-default; React Suspense/loading states handle the microtask delay naturally. Matches Tauri IPC behavior.                                                                                                                                          |

### Integration Challenges

- **`convertFileSrc` is synchronous**: Unlike `invoke`, `convertFileSrc` cannot be wrapped in a dynamic import. It must be resolved eagerly at startup via an `initConverters()` call in `main.tsx` or module-top level. Two approaches documented in `research-technical.md` — either eagerly re-export in `lib/plugin-stubs/convertFileSrc.ts` or use a top-level `await` at module init.
- **`ProfileSummary` not exported**: Local to `hooks/useLibrarySummaries.ts`. Must be moved to `types/library.ts` as part of Phase 1 before typed mock handlers for `profile_list_summaries` can compile cleanly.
- **`utils/optimization-catalog.ts` caches `invoke()` at module level**: Non-hook, non-component usage. Must migrate to `callCommand` the same as everything else.
- **Vite HMR resets the mock singleton**: Intended behavior. Contributors editing `handlers/profile.ts` will see their changes reflected, but the `store.ts` state also resets — which may surprise contributors iterating on multi-step profile-mutation flows.
- **`App.tsx` has a direct `listen('onboarding-check', ...)` call at line 67**: Migrates to `subscribeEvent('onboarding-check', ...)`. The onboarding wizard never opens in browser mode unless `?onboarding=show` is used, because nothing emits the event.
- **13 components call `invoke()` directly instead of through hooks**: Architectural smell flagged by practices-researcher. The browser-mode migration rewrites them mechanically to `callCommand` but does not fix the smell. Tracked as a follow-up `refactor:` issue.
- **Banner CSS placement**: `dev-indicator.css` must be imported inside a `__WEB_DEV_MODE__` branch at module level (or via a webdev-only Vite entry) so it never enters the production stylesheet. The class name `.crosshook-app--webdev` never appears in production bundles.

### Security Considerations

#### Critical — Hard Stops

| Finding                                    | Risk                                                                                                                                                     | Required Mitigation                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Production AppImage contains mock code     | Tauri WebView silently calls mock data instead of real Rust IPC, masking real bugs and delivering a false sense of a working application                 | **Three-layer defense**: (1) `__WEB_DEV_MODE__` Vite `define` = `false` in production, (2) dynamic `import('./mocks')` inside the dead branch, (3) CI `verify:no-mocks` grep sentinel in `release.yml` on `dist/assets/*.js` for `[dev-mock]`, `getMockRegistry`, `registerMocks`, `MOCK MODE`. **The CI grep is the authoritative control** — the guard is primary, grep is fail-safe. (W-1 in research-security.md) |
| Vite dev server exposed beyond `127.0.0.1` | Contributors on shared networks (office, conference Wi-Fi) expose source files, HMR WebSocket, and fixture data containing system layout info to the LAN | Force `server.host = '127.0.0.1'` and `server.strictPort = true` in `vite.config.ts` webdev branch; document `--host 0.0.0.0` as unsupported in `AGENTS.md` and `scripts/dev-native.sh` help. (W-2)                                                                                                                                                                                                                   |
| Real secrets in committed fixtures         | Fixture files may accidentally include API keys, tokens, real user IDs, Steam IDs, home paths scraped from developer machines                            | **Content policy (mandatory)**: Fixtures use obviously-synthetic values only (`Test Game Alpha`, Steam IDs ≥ `9999001`, `/mock/game/game.exe`); policy documented at the top of `lib/mocks/index.ts` and in `CONTRIBUTING.md`. PR review checklist enforces. Phase 3+: CI grep scoped to `lib/mocks/fixtures/` for SteamID64 pattern + home paths. (W-3)                                                              |

#### Warnings — Must Address

| Finding                                                                                                   | Risk                                                                                                                                                     | Mitigation                                                                                                                                                                                                                           | Alternatives                                                                                                                                                                                            |
| --------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Dynamic `import()` of mock modules not automatically tree-shaken (Vite/Rolldown limitation, issue #11080) | Rollup tracks dynamic imports for chunk-graph construction before dead-code elimination; mock chunk may still be emitted as unreferenced file in `dist/` | Use `__WEB_DEV_MODE__` `define` constant (not `import.meta.env.MODE` string) so Rollup sees `if (false) { import(...) }` and drops the dynamic import. Combined with CI grep sentinel.                                               | `import.meta.env.MODE === 'webdev'` — weaker chunk-graph guarantee but lower misconfiguration risk (no extra `define` config needed, `MODE` set automatically by `--mode`). CI grep covers either case. |
| `dev:browser` script silently fails if `--mode webdev` is omitted                                         | `__WEB_DEV_MODE__` stays `false`, adapter tries to call real `invoke()` with no Tauri bridge, throws confusing error                                     | Hard-code `--mode webdev` in the `"dev:browser"` npm script; document the dependency in `AGENTS.md` and `lib/mocks/README.md`; consider a runtime assertion at the top of `ensureMocks()` that throws if `__WEB_DEV_MODE__` is false | Use `import.meta.env.MODE === 'webdev'` string comparison (lower footgun, weaker dead-code elimination)                                                                                                 |

#### Advisories — Best Practices

- **A-1 — Centralize mode checks in `lib/ipc.ts` only** (ESLint `no-restricted-imports` blocking `**/lib/mocks/**` from anywhere except `**/lib/ipc.ts`): architectural discipline to prevent mock data leaking through component-level conditionals. Deferral justification: trivially adjustable in any future PR; enforcement can be added once the adapter is stable.
- **A-2 — Skip MSW entirely, keep `public/` clean**: the hand-rolled adapter has no postinstall side effects; MSW's `mockServiceWorker.js` would otherwise bloat `dist/`. Deferral: already addressed by library rejection.
- **A-3 — Avoid `@faker-js/faker`**: historical sabotage precedent. Static TS fixtures are sufficient. Deferral: only re-evaluate if fixture generation ever exceeds hand-authoring.
- **A-4 — Document `isTauri()` as a dev-routing heuristic, not a security boundary**: the real security boundary is Tauri's capability system on the Rust side. Deferral: doc-only.
- **A-5 — Source maps remain disabled in webdev mode**: `vite.config.ts` currently sets `sourcemap: isDebug` where `isDebug = !!process.env.TAURI_ENV_DEBUG`. Plain `vite` does not set this; source maps are off in browser dev mode by default. Constraint: any future `vite.config.webdev.ts` override MUST keep `sourcemap: false`. Deferral: no code change needed; document as constraint.
- **A-6 — Non-dismissable two-layer dev-mode indicator**: already required by business rules and UX design; crop-resistant amber inset outline + corner chip. No deferral.

---

## Task Breakdown Preview

### Phase 1: Foundation (single PR)

**Focus**: Ship the adapter layer, mechanical call-site migration, boot-critical handlers, dev-mode indicator, and CI safety gate in one coherent change so the app boots past the loading screen at `http://localhost:5173`.

**Tasks**:

1. **Script & build config**
   - Add `--browser` / `--web` flag branch to `scripts/dev-native.sh`
   - Add `"dev:browser": "vite --mode webdev"` to `src/crosshook-native/package.json`
   - Extend `vite.config.ts` with mode-conditional `define: { __WEB_DEV_MODE__: mode === 'webdev' }` and `server.host: '127.0.0.1'` + `strictPort: true` for webdev mode
   - Declare `const __WEB_DEV_MODE__: boolean` in `src/crosshook-native/src/vite-env.d.ts`

2. **Adapter layer creation**
   - `src/crosshook-native/src/lib/runtime.ts` — `isTauri()` probe
   - `src/crosshook-native/src/lib/ipc.ts` — `callCommand<T>` with dynamic mock import + unhandled-command error
   - `src/crosshook-native/src/lib/events.ts` — `subscribeEvent<T>` + `emitMockEvent` + in-process `browserBus`

3. **Plugin stubs**
   - `src/crosshook-native/src/lib/plugin-stubs/dialog.ts` (`open`, `save`; `console.warn` + `null` in browser)
   - `src/crosshook-native/src/lib/plugin-stubs/shell.ts` (`open`, `execute` throws)
   - `src/crosshook-native/src/lib/plugin-stubs/fs.ts` (`readFile` stub, `writeFile` throws)
   - `src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts` (passthrough in browser)

4. **Mock registry skeleton**
   - `src/crosshook-native/src/lib/mocks/index.ts` — `registerMocks()` orchestrator
   - `src/crosshook-native/src/lib/mocks/store.ts` — `MockStore` + `getStore()`
   - `src/crosshook-native/src/lib/mocks/eventBus.ts` — glue to `lib/events.ts`
   - `src/crosshook-native/src/lib/mocks/README.md` — contributor guide with one worked example

5. **Boot-critical handlers (minimum to render past loading screen)**
   - `handlers/settings.ts` — `settings_load`, `recent_files_load`, `settings_save`, `recent_files_save`, `default_steam_client_install_path`
   - `handlers/profile.ts` — `profile_list`, `profile_load`, `profile_list_summaries`, `profile_list_favorites`

6. **Dev-mode indicator**
   - `src/crosshook-native/src/lib/DevModeBanner.tsx` (or `DevModeChip.tsx`) — Layer 2 corner chip component
   - `src/crosshook-native/src/lib/dev-indicator.css` — Layer 1 inset outline + chip sizing overrides
   - Wire into `App.tsx` via `__WEB_DEV_MODE__` class + conditional chip render

7. **Mechanical migration (~50 files)**
   - Find/replace `import { invoke } from '@tauri-apps/api/core'` → `import { callCommand } from '@/lib/ipc'`
   - Find/replace `invoke(` → `callCommand(`
   - Find/replace `import { listen } from '@tauri-apps/api/event'` → `import { subscribeEvent } from '@/lib/events'`
   - Find/replace `listen(` → `subscribeEvent(`
   - Migrate all `@tauri-apps/plugin-dialog`, `-shell`, `-fs` imports to `lib/plugin-stubs/`
   - Migrate `utils/optimization-catalog.ts` non-hook `invoke` callsite
   - Export `ProfileSummary` from `src/crosshook-native/src/types/library.ts` (currently local to `hooks/useLibrarySummaries.ts`)

8. **CI safety gate**
   - Add `verify:no-mocks` grep step to `.github/workflows/release.yml` after the Build native AppImage step

9. **Documentation**
   - Update `AGENTS.md` "Commands" section to mention `./scripts/dev-native.sh --browser`
   - Add a 1-paragraph "Browser Dev Mode" section pointing to `lib/mocks/README.md`

10. **Verification**
    - `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` passes
    - Manual smoke: launch `./scripts/dev-native.sh --browser`, navigate all 9 routes, verify dev indicator on every route, verify no console errors
    - Manual smoke: launch `./scripts/dev-native.sh` (Tauri mode), verify production build still works identically, verify dev indicator is ABSENT
    - Manual smoke: `./scripts/build-native.sh --binary-only`; verify `verify:no-mocks` grep passes locally before relying on CI

**Parallelization**: sub-tasks 2, 3, 4, 5, 6 can be developed in parallel. Task 7 (mechanical migration) must wait for 2, 3 to land. Tasks 8, 9 are leaf tasks.

---

### Phase 2: Handler Fan-out

**Focus**: Expand mock handler coverage to all ~18 domain groups so every major route renders meaningful data. Each sub-phase can be its own PR.

**Dependencies**: Phase 1 complete and merged.

**Tasks** (ordered by iteration value, not codebase order):

- **2.1 Launch** — `launch_game`, `launch_trainer`, `preview_launch`, `validate_launch`, `check_game_running`, `verify_trainer_hash`, `check_gamescope_session` + events `launch-log`, `launch-diagnostic`, `launch-complete`, `profiles-changed`. Emit events from within mutating handlers via `emitMockEvent`.
- **2.2 Profiles** — `profile_save`, `profile_duplicate`, `profile_rename`, `profile_delete`, `profile_set_favorite`, config history/diff/rollback commands, optimization preset commands. Round-trip via `store.ts`; emit `profiles-changed` on mutations.
- **2.3 Health dashboard** — `batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`, `check_version_status`, `acknowledge_version_change` + events `profile-health-batch-complete`, `version-scan-complete`.
- **2.4 Onboarding** — `check_readiness`, `dismiss_onboarding`, `check_version_status` + events `onboarding-check`, `auto-load-profile`. Gated on `?onboarding=show` query param.
- **2.5 Install / Update flows** — `install_game`, `validate_install_request`, `update_game`, `validate_update_request`, `cancel_update` + events `install-*`, `update-*`.
- **2.6 Proton stack** — `list_proton_installs`, `check_proton_migrations`, `apply_proton_migration`, `apply_batch_migration`.
- **2.7 ProtonDB** — `protondb_lookup`, `protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion`.
- **2.8 ProtonUp** — `protonup_list_available_versions`, `protonup_install_version`, `protonup_get_suggestion`.
- **2.9 Community** — `community_list_profiles`, `community_list_indexed_profiles`, `community_sync`, `community_add_tap`, `community_prepare_import`, `community_export_profile`.
- **2.10 Launcher export** — `list_launchers`, `check_launcher_exists`, `validate_launcher_export`, `export_launchers`, `preview_launcher_*`, `delete_launcher*`.
- **2.11 Discovery, run-executable, prefix storage, diagnostics, catalog, art, steam** — smaller groups, may collapse into one PR.

**Parallelization**: Phase 2 sub-PRs are independent; multiple can be in flight simultaneously.

---

### Phase 3: Polish

**Focus**: Fixture variants, orthogonal debug toggles, coverage tooling, docs, optional test automation.

**Dependencies**: Phase 2 "Profiles" and "Launch" handlers merged so fixture variants have enough surface area to be meaningful.

**Tasks**:

- **3.1 Fixture state switcher** — `?fixture=populated|empty|error|loading` URL param; fixture-state-aware dispatch in `registerMocks()`. Full reload on change (documented in UX research).
- **3.2 Orthogonal debug toggles** — `?errors=true` (rejects write commands only), `?delay=<ms>` (wraps all responses in `setTimeout`), `?onboarding=show` (synthesizes `onboarding-check` event on mount).
- **3.3 Handler-coverage check script** — `dev:browser:check` npm script scans `crosshook-core` Rust command sources and lists unmocked commands. Optional; Phase 3 nice-to-have.
- **3.4 Project README update** — document the dev-browser flow, link `lib/mocks/README.md`.
- **3.5 Optional Playwright smoke test** — boot the dev server and screenshot-check every route in each fixture state. Gates future visual regression suite.
- **3.6 Follow-up `refactor:` issue** — track the 13 components that call `invoke()` directly instead of through hooks.
- **3.7 Fixture-content CI lint (scoped to `lib/mocks/fixtures/`)** — grep for SteamID64 pattern `\b[0-9]{17}\b` and home paths `/home/`, `/users/`, `/Users/` scoped to string literals in TS/JSON files.

**Parallelization**: All Phase 3 tasks are independent; can ship as separate small PRs.

---

## Resolved Decisions

All six decisions confirmed 2026-04-07 — team-lead accepted the research-recommendations wholesale. These are settled; `/plan-workflow` should treat them as given, not re-litigate them.

| #   | Decision                      | Resolution                                                                                                                                                 | Impact on Phase 1                                                                                                                                                                                                                    |
| --- | ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D1  | Flag name for `dev-native.sh` | **`--browser`** (primary) with **`--web`** as alias; `--dev` rejected as too generic                                                                       | `scripts/dev-native.sh` case branch matches `--browser\|--web`; help text and `AGENTS.md` use `--browser` as the canonical form                                                                                                      |
| D2  | PR structure for Phase 1      | **Single PR** — foundation + plugin stubs + adapter + mechanical migration + boot-critical handlers + CI gate shipped together                             | Size the PR carefully; coordinate reviewer allocation; do not split adapter from migration                                                                                                                                           |
| D3  | Build-time guard              | **`__WEB_DEV_MODE__`** Vite `define` constant gated on `mode === 'webdev'`                                                                                 | `vite.config.ts` adds mode-conditional `define`; `"dev:browser": "vite --mode webdev"` hard-codes the mode; runtime assertion at top of `ensureMocks()` throws if `__WEB_DEV_MODE__` is `false` at the time of first mock invocation |
| D4  | Plugin stub semantics         | **`null` + loud `console.warn`** for `dialog.open`/`save`; **`throw`** for destructive ops (`shell.execute`, `fs.writeFile`, `fs.removeFile`, `fs.rename`) | `lib/plugin-stubs/dialog.ts` returns `null` and logs; `lib/plugin-stubs/shell.ts` and `lib/plugin-stubs/fs.ts` throw on destructive paths; optional toast deferred to Phase 3                                                        |
| D5  | `@/` Vite path alias          | **Yes — add in Phase 1**                                                                                                                                   | `vite.config.ts` adds `resolve.alias: { '@': path.resolve(__dirname, './src') }`; migration rewrites use `@/lib/ipc`, `@/lib/events`, `@/lib/plugin-stubs/*` instead of long relative paths; update `tsconfig.json` paths to match   |
| D6  | CSP for browser dev mode      | **Accept Chrome/Firefox defaults** for Phase 1; do not mirror the production CSP in Vite middleware                                                        | Document in `lib/mocks/README.md` that "UI must be re-verified in `./scripts/dev-native.sh` before merge"; revisit in Phase 3 if WebKit-only regressions become recurring                                                            |

### Derived Phase 1 requirements from resolved decisions

These follow mechanically from D1–D6 and should be added to the Phase 1 task breakdown when generating the plan:

- **From D3:** `ensureMocks()` starts with `if (!__WEB_DEV_MODE__) throw new Error('[dev-mock] mock layer invoked in non-webdev build — check dev:browser script passes --mode webdev')`. This is the misconfiguration safety net.
- **From D5:** `tsconfig.json` gets a matching `paths` entry: `"@/*": ["./src/*"]`. Without this, TypeScript will reject the new `@/` imports even if Vite resolves them correctly.
- **From D5:** the mechanical migration uses `@/lib/ipc`, `@/lib/events`, `@/lib/plugin-stubs/dialog`, `@/lib/plugin-stubs/shell`, `@/lib/plugin-stubs/fs`, `@/lib/plugin-stubs/convertFileSrc` as import specifiers — not relative paths.
- **From D4:** `lib/plugin-stubs/fs.ts` implements read operations (`readTextFile`, `readFile`, `exists`, `metadata`) as resolving stubs that return synthetic data; write/delete operations (`writeFile`, `writeTextFile`, `removeFile`, `removeDir`, `rename`, `createDir`) throw. Same split for `shell.ts`: `open` (opens URL) resolves as a no-op + warn; `execute`, `Command.spawn` throw.
- **From D1:** `scripts/dev-native.sh` help text in the `usage()` heredoc documents both `--browser` and `--web` as equivalent, with a note that `--host 0.0.0.0` is unsupported in this mode.

---

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): External API research — `@tauri-apps/api/mocks`, `isTauri()`, MSW rejection, faker rejection, prior art, full `mockIPC` vs `callCommand` trade-off table, Round 1/2/3 corrections.
- [research-business.md](./research-business.md): Business logic analysis — 8 user stories (US-1 through US-8), 12 business rules (BR-1 through BR-12), 4 architectural decisions (AD-1 through AD-4), full IPC call inventory, storage boundary classification, 12 success criteria.
- [research-technical.md](./research-technical.md): Technical specifications — full file inventory, per-module implementation snippets, plugin stub pattern, mock registry pattern, `convertFileSrc` synchrony edge case, boot sequence, tree-shaking strategy, CI sentinel check, `vite.config.ts` assessment.
- [research-ux.md](./research-ux.md): UX research — two-layer dev-mode indicator spec, sidebar layout analysis (rejected `crosshook-sidebar__brand` and status-group; chose `position: fixed` corner), Storybook/MSW/Vercel/GitHub/Linear indicator patterns reviewed, fixture-toggle query-string pattern, browser-vs-Tauri parity gotchas, element inspection workflow, accessibility of dev affordances.
- [research-security.md](./research-security.md): Security analysis — severity-leveled findings (CRITICAL: mock-code production leak, dev-server LAN exposure, fixture secret content; WARNING: dynamic-import tree-shaking limitation W-1, dev-server host W-2, fixture PII W-3; ADVISORY: A-1 through A-6), dependency security summary (zero new deps), authentication N/A, infrastructure security, trade-off recommendations.
- [research-practices.md](./research-practices.md): Engineering practices — existing reusable code table, 4-module modularity design, KISS assessment, rule-of-three verdicts, interface design with typed `Commands` map as additive improvement, testability patterns, build-vs-depend table with `mockIPC` correction, 84-call-site migration scope.
- [research-recommendations.md](./research-recommendations.md): Full synthesis — executive summary, recommended approach with full file layout, code snippets, 3-phase task breakdown, improvement ideas, 6-entry alternative approaches table, risk mitigation summary table, persistence boundary statement, 9 key decisions needed, 9 open questions.
