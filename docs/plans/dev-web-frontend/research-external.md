# External Libraries & Integration Patterns: dev-web-frontend Mode

_Research conducted: 2026-04-07_
_Corrected: 2026-04-07 (round 1, round 2, round 3) — see Corrections section_

---

## Executive Summary

CrossHook's React/Vite frontend calls `@tauri-apps/api invoke()` which only resolves inside the Tauri WebView. The goal is a `--dev` flag that runs only the Vite dev server so designers can iterate in a normal browser with DevTools, logs, and mock data — without changing production behavior.

**Recommended approach** (updated after two rounds of codebase verification): A **hand-rolled `callCommand` IPC adapter** (`lib/ipc.ts`) that routes to either the real `invoke` or a plain function-map of mock handlers. No new external libraries are required.

`mockIPC` from `@tauri-apps/api/mocks` is a legitimate alternative with zero callsite changes, but has a fatal interaction with `isTauri()` detection and relies on silent `undefined` returns for unregistered commands (see `mockIPC` section below for full trade-off table).

Key validated findings:

- `mockIPC` **does** work in a pure browser context — it calls `mockInternals()` first, which initializes `window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ ?? {}`. It does not require a running Tauri process. (Round 1 retraction was itself wrong — retracted in round 2.)
- **However**, calling `mockIPC` in browser mode sets `window.__TAURI_INTERNALS__`, which makes the `isTauri()` probe return `true`. This means `isTauri()` cannot be used as a runtime guard if `mockIPC` is the chosen approach — the guard must be a different signal (e.g., `import.meta.env.DEV`).
- `import.meta.env.DEV` (or a `define`-based compile-time constant) is the correct Vite build-time guard. It is always defined and statically replaced at build time. Bundle exclusion of mock code must be verified by a CI bundle check — the guard is necessary but not sufficient on its own if dynamic imports are involved.
- `isTauri()` from `@tauri-apps/api/core` checks `globalThis.isTauri` (not `__TAURI_INTERNALS__`) as of v2.5.0 — so it returns `false` in a plain browser even after `mockIPC` sets `__TAURI_INTERNALS__`. This makes `isTauri()` safe to import from the library for the detection probe, but the hand-rolled `lib/runtime.ts` using `__TAURI_INTERNALS__` would be poisoned by `mockIPC`.
- `DEFAULT_APP_SETTINGS` and `createDefaultProfile()` already exist in `types/settings.ts` and `types/profile.ts` — ready-made fixtures for the most-called commands.
- MSW cannot intercept Tauri IPC — operates at HTTP/XHR layer, wrong tool. Rejected.
- `@faker-js/faker` v10 is unnecessary — static TypeScript fixtures are sufficient for design iteration.

**Confidence**: High. Source-verified against `@tauri-apps/api@2.10.1` published package.

---

## Corrections (post codebase review)

### Round 1 corrections (practices-researcher codebase analysis)

| Retracted claim                                                | Correct finding                                                                                     |
| -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| "MSW v2 is useful when the UI also makes `fetch()` calls"      | CrossHook calls `invoke()` exclusively (84 call sites). MSW operates at the HTTP layer. Wrong tool. |
| "`@tauri-apps/api/mocks` is the primary recommended mechanism" | Hand-rolled `callCommand` adapter is preferred. See full trade-off table below.                     |

### Round 2 correction (practices-researcher source verification against `@tauri-apps/api@2.10.1`)

| Round 1 retraction that was itself wrong                                                  | Actual finding                                                                                                                                                                                                    |
| ----------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| "`mockIPC` requires the Tauri bridge to be present; it cannot function in a pure browser" | **Wrong.** `mockIPC` calls `mockInternals()` which does `window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ ?? {}` — it initializes the object if absent. `mockIPC` **does** work in a pure browser context. |

The correct reason to prefer `callCommand` over `mockIPC` is not that `mockIPC` doesn't work — it's that:

1. `mockIPC` sets `window.__TAURI_INTERNALS__`, poisoning any `__TAURI_INTERNALS__`-based `isTauri()` probe
2. `mockIPC` returns `undefined` silently for unregistered commands unless the caller explicitly throws
3. `callCommand` throws explicitly on missing mocks by design

Source: `docs/plans/dev-web-frontend/research-practices.md` and direct source inspection of `@tauri-apps/api@2.10.1`.

### Round 3 corrections (security-researcher review)

| Claim                                                                                                            | Correction                                                                                                                                                                                                                                                                           |
| ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `lib/runtime.ts` probe used `typeof window.__TAURI_INTERNALS__ !== 'undefined'`                                  | Wrong probe — poisoned by `mockIPC`. Correct probe: `!!((globalThis as unknown as ...).isTauri)`, matching the official library implementation since v2.5.0.                                                                                                                         |
| "`mockIPC` viable for browser dev mode with `import.meta.env.DEV` guard"                                         | Partially correct but incomplete. `mockIPC` is designed for test runners (Vitest/Jest) with `clearMocks()` between test runs — not persistent app sessions. For browser dev mode it needs a manually maintained persistent registry wrapper, which reconstructs `callCommand` logic. |
| "Vite statically replaces `import.meta.env.DEV`, ensuring mock branches are tree-shaken from production bundles" | Overstated. The guard is necessary but not sufficient. A `define`-based compile-time constant and a `verify:no-mocks` CI bundle check are the authoritative guarantee. Do not state exclusion as a given based on the guard alone.                                                   |

Source: `docs/plans/dev-web-frontend/research-security.md` (security-researcher review).

---

## Primary APIs

### `isTauri()` — Runtime Detection (Tauri v2)

**No library required for a standalone probe.**

The official `isTauri()` from `@tauri-apps/api/core` (v2.5.0+) checks `globalThis.isTauri`, not `window.__TAURI_INTERNALS__`. The correct hand-rolled probe is:

```typescript
// src/crosshook-native/src/lib/runtime.ts
export function isTauri(): boolean {
  return !!(globalThis as unknown as Record<string, unknown>).isTauri;
}
```

**Why not `__TAURI_INTERNALS__`**: using `typeof window.__TAURI_INTERNALS__ !== 'undefined'` as the probe is poisoned if `mockIPC` is ever called — `mockIPC` initializes `window.__TAURI_INTERNALS__ = {}` unconditionally. The `globalThis.isTauri` probe is not affected by `mockIPC`.

**Why a standalone module**: avoids a potential circular dependency if `lib/ipc.ts` also imports from `@tauri-apps/api/core`. Either approach (standalone or direct library import) works — the standalone probe is explicit about what it checks.

**Reference**: [GitHub discussion #6119](https://github.com/tauri-apps/tauri/discussions/6119), [`@tauri-apps/api` v2.5.0 release](https://v2.tauri.app/release/)

**Confidence**: High — verified against `@tauri-apps/api@2.10.1` source.

---

### `@tauri-apps/api/mocks` — Tauri v2 Official Mock System

**Docs**: [https://v2.tauri.app/develop/tests/mocking/](https://v2.tauri.app/develop/tests/mocking/)
**API reference**: [https://v2.tauri.app/reference/javascript/api/namespacemocks/](https://v2.tauri.app/reference/javascript/api/namespacemocks/)
**Ships with**: `@tauri-apps/api ^2.0.0` — already in `package.json`, no new dependency.

**TypeScript signatures (exact):**

```typescript
// Intercept all invoke() calls. Also initializes window.__TAURI_INTERNALS__ if absent.
function mockIPC(cb: (cmd: string, payload?: InvokeArgs) => unknown, options?: MockIPCOptions): void;

// Mock file-path-to-URL conversion for a specific OS
function mockConvertFileSrc(osName: string): void;

// Inject fake window labels so @tauri-apps/api/window works
function mockWindows(current: string, ..._additionalWindows: string[]): void;

// Reset all injected mocks
function clearMocks(): void;

interface MockIPCOptions {
  shouldMockEvents?: boolean; // also mocks listen() / emit() if true
}
```

**How `mockIPC` initializes the browser environment** (verified against `@tauri-apps/api@2.10.1`):

`mockIPC` calls `mockInternals()` first, which does:

```javascript
window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ ?? {};
```

It creates the object if absent. This means `mockIPC` **works in a pure browser context** — it does not require a running Tauri process. The full call path becomes:

```
import invoke from @tauri-apps/api/core
  → window.__TAURI_INTERNALS__.invoke  (set by mockIPC)
    → mockIPC callback
      → mock registry handler
```

**The `isTauri()` detection conflict**: calling `mockIPC` sets `window.__TAURI_INTERNALS__`, which means any probe based on `typeof window.__TAURI_INTERNALS__ !== 'undefined'` returns `true` after mocking — poisoning detection. The `isTauri()` export from `@tauri-apps/api/core` checks `globalThis.isTauri` (not `__TAURI_INTERNALS__`), so it correctly returns `false` in a browser even after `mockIPC` runs. If using `mockIPC`, the production guard must be `import.meta.env.DEV` — NOT a `__TAURI_INTERNALS__` probe.

**Return value serialization**: none — values pass by reference. `payload` is `InvokeArgs | undefined` (argument-less commands receive `undefined`). Promises are awaited before returning to callers.

**Silent failure risk**: if the callback returns `undefined` for an unknown command, `invoke()` resolves to `undefined` rather than throwing. This produces confusing null-dereferences downstream rather than a clear error. Mitigate by throwing explicitly in the callback for unregistered commands.

**`mockConvertFileSrc`**: handles the `convertFileSrc` stub problem directly — mocks the OS-specific path-to-URL conversion formula (e.g., `'linux'`, `'windows'`).

**`emitTo` limitation**: bidirectional webview-to-webview event emission is not supported by this mock implementation.

**Intended use case**: Vitest/jsdom unit tests and integration tests — environments where you call `mockIPC`, run assertions, then call `clearMocks()` between test cases. `clearMocks()` is designed to reset state between test runs, not between user interactions.

**Limitations for a live browser dev session**: `mockIPC` has no concept of a persistent mock registry across a full running app session. `mockWindows()` stubs only presence metadata, not window properties. There is no lifecycle integration with React rendering or HMR. If used for browser dev mode, the caller must manually maintain the mock registry in a wrapper (which reconstructs the same logic as `callCommand`), and the `import.meta.env.DEV` guard must prevent the mock bootstrap from reaching the production bundle.

**Viable for browser dev mode** only when: zero callsite changes are a hard constraint, the callback explicitly throws on unregistered commands, and the `import.meta.env.DEV` / `define`-constant guard is enforced and verified by a CI bundle check.

**Confidence**: High — verified against published source.

---

### Hand-rolled IPC Adapter — Recommended Pattern

No library. Four modules. See `research-practices.md` for full detail.

```typescript
// src/crosshook-native/src/lib/ipc.ts
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

Key properties:

- Dynamic import of `@tauri-apps/api/core` is under the `isTauri()` runtime branch — `isTauri()` handles routing, not bundle exclusion. Bundle exclusion of the mock branch requires the `__WEB_DEV_MODE__` compile-time `define` constant; verified by the `verify:no-mocks` CI gate
- Fails fast on missing mock (no silent `undefined` returns)
- Generics at call site (`callCommand<T>`) — same shape as existing `invoke<T>` calls, minimal migration friction
- 84 `invoke()` call sites across 35 files need to be migrated to `callCommand()`

---

### Vite Build-time Guard

```typescript
// import.meta.env.DEV is statically replaced at build time by Vite
// → entire block is dead code in `vite build` output
if (import.meta.env.DEV) {
  // safe to dynamically import mock-only modules here
}
```

**Reference**: [Vite env variables and modes](https://vite.dev/guide/env-and-mode)

**Important caveat on bundle exclusion**: `import.meta.env.DEV` becoming `false` at build time is necessary but not sufficient to guarantee mock code is absent from production bundles. If the mock bootstrap uses a dynamic `import('./mocks')` inside the guarded branch, Rollup may still include the mock module in the chunk graph before dead code elimination runs.

The reliable guarantee is:

1. Use a `define`-based compile-time boolean constant (e.g., `__WEB_DEV_MODE__: false` in `vite.config.ts` `define` for production builds) rather than relying solely on `import.meta.env.DEV`
2. Place the dynamic `import()` inside the guarded branch (never at module scope)
3. Verify with a `verify:no-mocks` CI bundle check — the security doc treats this gate as the authoritative guarantee, not the guard alone

Do NOT state mock exclusion as a given based on the guard alone. Reference the CI verification step as the proof.

**`VITE_*` undefined variable caveat**: custom `VITE_*` vars that are not defined are NOT tree-shaken (Vite issue [#15256](https://github.com/vitejs/vite/issues/15256)). Use `import.meta.env.DEV` or a `define` constant — both are always defined — as the primary guard.

---

## Integration Patterns

### Pattern A: `callCommand` adapter (recommended)

No bootstrap call needed. `isTauri()` (via `globalThis.isTauri`) is checked inline on each call. Mock registry loads lazily on the first non-Tauri invocation.

```typescript
// src/crosshook-native/src/lib/ipc.ts
import { isTauri } from '@tauri-apps/api/core';
import { getMockRegistry } from './mocks/index';

export async function callCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    const { invoke } = await import('@tauri-apps/api/core');
    return invoke<T>(command, args);
  }
  const handler = getMockRegistry()[command];
  if (!handler) throw new Error(`[dev-mock] No mock for: ${command}`);
  return handler(args) as Promise<T>;
}
```

Note: using `isTauri()` from `@tauri-apps/api/core` (checks `globalThis.isTauri`) is correct here. Do NOT use a `__TAURI_INTERNALS__` probe in the adapter if `mockIPC` might ever be called — but `callCommand` and `mockIPC` are mutually exclusive choices.

### Pattern B: `mockIPC` bootstrap (alternative — zero callsite changes)

If zero callsite changes are required, call `mockIPC` in `main.tsx` before the React tree renders. Production guard must be `import.meta.env.DEV` (not `isTauri()`, since `mockIPC` poisons `__TAURI_INTERNALS__`):

```typescript
// src/crosshook-native/src/main.tsx
if (import.meta.env.DEV) {
  const { mockIPC, mockWindows, mockConvertFileSrc } = await import('@tauri-apps/api/mocks');
  const { handlers } = await import('./mocks/ipc-handlers');
  mockWindows('main');
  mockConvertFileSrc('linux');
  mockIPC((cmd, payload) => {
    const handler = handlers[cmd];
    if (!handler) throw new Error(`[dev-mock] No mock for: ${cmd}`);
    return handler(payload);
  }, { shouldMockEvents: true });
}
ReactDOM.createRoot(document.getElementById('root')!).render(<App />);
```

`mockConvertFileSrc('linux')` resolves the `convertFileSrc` stub problem directly. `shouldMockEvents: true` covers basic `listen()` interception.

### Pattern C: `listen()` stub (for `callCommand` path)

`listen` from `@tauri-apps/api/event` appears in ~10 files. For the `callCommand` approach, provide a stub that maintains a real in-memory listener map (not a pure no-op) so event-driven UI can be exercised with `emitMockEvent`:

```typescript
// lib/mocks/tauri-events.ts
type EventCallback<T> = (event: { payload: T }) => void;
const listeners = new Map<string, Set<EventCallback<unknown>>>();

export function listenStub<T>(event: string, handler: EventCallback<T>): Promise<() => void> {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(handler as EventCallback<unknown>);
  return Promise.resolve(() => listeners.get(event)?.delete(handler as EventCallback<unknown>));
}

export function emitMockEvent<T>(event: string, payload: T): void {
  listeners.get(event)?.forEach((cb) => cb({ payload }));
}
```

### Pattern D: Plugin stubs (both paths)

All three plugins are declaration-only at module level — safe to import in a browser. Only the function calls throw. Guard call sites with `if (isTauri())` or provide no-op wrappers:

```typescript
// lib/mocks/tauri-plugins.ts
export const dialogStub = {
  open: async () => null,
  save: async () => null,
};
export const shellStub = {
  open: async (_path: string) => undefined,
};
```

### Pattern E: `convertFileSrc` (for `callCommand` path)

```typescript
// lib/mocks/tauri-utils.ts
export function convertFileSrcStub(path: string, _protocol = 'asset'): string {
  return path; // passthrough — cover art from local paths won't display, but no throw
}
```

For the `mockIPC` path, use `mockConvertFileSrc('linux')` instead.

---

## Libraries and SDKs

| Library                                      | Current version            | New dep?     | Verdict                                                                                                                                |
| -------------------------------------------- | -------------------------- | ------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| `@tauri-apps/api/mocks` (`mockIPC`)          | ships with `^2.0.0`        | No           | Viable — works in browser, zero callsite changes, but poisons `__TAURI_INTERNALS__` detection; use `import.meta.env.DEV` guard instead |
| `@tauri-apps/api/core` (`invoke`, `isTauri`) | `^2.0.0` (already in deps) | No           | `isTauri()` checks `globalThis.isTauri` — safe to use even after `mockIPC` runs                                                        |
| `msw`                                        | 2.12.14                    | Would be new | Rejected — wrong interception layer; postinstall hook writes to `dist/`; CrossHook has no `fetch()` calls                              |
| `@faker-js/faker`                            | 10.4.0                     | Would be new | Rejected for initial feature — static TS fixtures sufficient; pin exact version if ever adopted                                        |
| Storybook 8                                  | 8.x                        | Would be new | Out of scope for initial feature                                                                                                       |
| Ladle                                        | latest                     | Would be new | Out of scope; preferred over Storybook if component isolation is later desired                                                         |

**Net new dependencies required**: zero for either approach.

---

## Prior Art

### 1. `tauri-remote-ui` — DraviaVemal

**Repo**: [https://github.com/draviavemal/tauri-remote-ui](https://github.com/draviavemal/tauri-remote-ui)

A Tauri plugin that exposes the running app's UI to a web browser while the native app keeps running. Requires the full Tauri process to be running — not suitable for the use case here (pure browser, no Tauri process).

### 2. Community discussion: browser-only dev with hand-rolled mocks

**Discussion**: [https://github.com/tauri-apps/tauri/discussions/10992](https://github.com/tauri-apps/tauri/discussions/10992)

Community members confirm the function-map approach: wrap `invoke()` in a custom function that checks for the Tauri environment and routes to mock handlers in browser context.

### 3. Vite middleware proxy pattern (DEV.to)

**Article**: [https://dev.to/purpledoubled/how-i-built-a-desktop-ai-app-with-tauri-v2-react-19-in-2026-1g47](https://dev.to/purpledoubled/how-i-built-a-desktop-ai-app-with-tauri-v2-react-19-in-2026-1g47)

Wraps `invoke()` in a `backendCall()` function: if `isTauri()` is false, maps command names to local Vite dev server middleware endpoints. A more complex variant of the same adapter pattern — relevant if the mock handlers need to do real computation (not needed for static fixture iteration).

---

## Storybook / Ladle / Histoire Trade-offs

| Tool        | Cold Start | React Support | Tauri mock needed               | Best for                            |
| ----------- | ---------- | ------------- | ------------------------------- | ----------------------------------- |
| Storybook 8 | ~8s        | Full          | Yes (manual mock in decorators) | Component library, design system    |
| Ladle       | ~1.2s      | React-only    | Yes (same)                      | Fast iteration, React-only projects |
| Histoire    | ~2s        | Vue 3         | N/A                             | Vue projects                        |

Neither replaces a full-app browser dev mode. Both require the same mock infrastructure (`callCommand` adapter) to handle `invoke()` calls within stories. If component isolation is later desired, **Ladle** is preferred for this React-only stack.

---

## `mockIPC` vs `callCommand` — Full Trade-off Table

| Dimension                           | `mockIPC` from `@tauri-apps/api/mocks`                                                                                                                                                               | `callCommand` adapter                                                                                                                                                         |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Works in pure browser               | Yes — initializes `__TAURI_INTERNALS__` itself                                                                                                                                                       | Yes                                                                                                                                                                           |
| Callsite changes                    | Zero — existing `invoke()` calls work unchanged                                                                                                                                                      | ~84 files (`invoke(` → `callCommand(`)                                                                                                                                        |
| New dependencies                    | Zero                                                                                                                                                                                                 | Zero                                                                                                                                                                          |
| `isTauri()` detection after mocking | `__TAURI_INTERNALS__`-based probe returns `true` (poisoned). Use `import.meta.env.DEV` guard instead. `globalThis.isTauri` probe (official `isTauri()`) still returns `false`.                       | Unaffected — detection is independent                                                                                                                                         |
| `convertFileSrc` stub               | `mockConvertFileSrc(osName)` handles it directly                                                                                                                                                     | Explicit `path => path` passthrough stub needed                                                                                                                               |
| `listen()` events                   | `shouldMockEvents: true` covers basic listen interception                                                                                                                                            | Separate `listenStub` needed                                                                                                                                                  |
| Missing mock error behavior         | Silent `undefined` unless callback explicitly throws                                                                                                                                                 | `throw new Error('[dev-mock] No mock for: X')` — loud by design                                                                                                               |
| Production bundle risk              | `mocks.js` must not reach the production bundle — requires `__WEB_DEV_MODE__` compile-time `define` constant + `verify:no-mocks` CI gate. `isTauri()` is runtime routing only, not bundle exclusion. | Same guarantee required. `isTauri()` routes correctly at runtime but does not exclude mock code from the bundle; the `define` constant + CI gate are the authoritative proof. |
| Discoverability                     | `invoke('foo')` silently returns mock data; requires knowing `mockIPC` was called at startup                                                                                                         | `callCommand('foo')` signals mock-ability at callsite                                                                                                                         |
| Structural complexity               | Registry dispatch still needed inside the callback — same logic as `callCommand`, just inline                                                                                                        | Explicit, centralized                                                                                                                                                         |

**Recommendation**: `callCommand` — explicit throws, no detection poisoning, no production import discipline required. `mockIPC` is viable if zero callsite changes are a hard constraint, but requires `import.meta.env.DEV` guard discipline and explicit throws in the callback.

---

## Constraints and Gotchas

1. **`mockIPC` poisons `__TAURI_INTERNALS__`-based `isTauri()` probes**: `mockIPC` initializes `window.__TAURI_INTERNALS__ = {}` if absent. Any probe using `typeof window.__TAURI_INTERNALS__ !== 'undefined'` will return `true` after `mockIPC` runs. If using `mockIPC`, the production guard must be `import.meta.env.DEV`. The official `isTauri()` from `@tauri-apps/api/core` checks `globalThis.isTauri` (not `__TAURI_INTERNALS__`) and still returns `false` in browser — but a hand-rolled `__TAURI_INTERNALS__` probe in `lib/runtime.ts` would be poisoned.

2. **MSW cannot intercept Tauri IPC**: Tauri `invoke()` is a WebView bridge call, not an HTTP request. MSW's Service Worker intercepts network-layer requests. The two systems operate at entirely different layers.

3. **`VITE_*` undefined variables do not tree-shake**: prefer `import.meta.env.DEV` for the primary production-safety guard (always defined, always static-replaced).

4. **84 `invoke()` call sites to migrate** (`callCommand` path only): 35 files. Mechanical find/replace plus import change per file.

5. **Plugin imports are declaration-only at module level** — safe to import in a browser. Throws only when functions are called. Stubs needed at call sites, not at import level.

6. **`listen()` events in browser mode**: with a no-op stub, event-driven UI (launch progress, console output) is static. With the `listenStub` + `emitMockEvent` pattern (Pattern C), events can be replayed from fixture code for richer iteration. Document which approach is in use.

7. **`ProfileSummary` type is not exported**: local to `hooks/useLibrarySummaries.ts`. Must be moved to `types/library.ts` before a typed mock fixture can import it.

---

## Open Questions

1. **`--browser` vs `--dev` flag in `dev-native.sh`**: the script has no such flag today. A `--browser` flag running `npm run dev` (plain Vite, no `tauri dev`) is a 3-line shell addition.

2. **Scope of event mocking**: if the launch flow UI or console output panel are important for design iteration, a synthetic event replay mechanism (emitting events at timed intervals from mock handlers) would be needed. Not required for the initial static fixture pass.

3. **`convertFileSrc` usage**: affects `useGameCoverArt.ts` and `MediaSection.tsx`. A no-op passthrough (`path => path`) is correct for local file paths; cover art from local files will simply not display in browser mode (no Tauri asset protocol).

4. **Typed `Commands` map**: additive improvement for autocomplete and type safety at call sites — not required for browser-dev mode to work. Can be added after the adapter is working.

---

## Sources

- [Tauri v2 Mocking Docs](https://v2.tauri.app/develop/tests/mocking/)
- [Tauri v2 Mocks API Reference](https://v2.tauri.app/reference/javascript/api/namespacemocks/)
- [Tauri v2 Stable Release Blog](https://v2.tauri.app/blog/tauri-20/)
- [GitHub Discussion: browser detection in Tauri #6119](https://github.com/tauri-apps/tauri/discussions/6119)
- [GitHub Discussion: mock Tauri API functions #10992](https://github.com/tauri-apps/tauri/discussions/10992)
- [Vite Env Variables and Modes](https://vite.dev/guide/env-and-mode)
- [Vite Tree-shaking env issue #15256](https://github.com/vitejs/vite/issues/15256)
- [MSW Quick Start](https://mswjs.io/docs/quick-start/)
- [MSW Browser Integration](https://mswjs.io/docs/integrations/browser/)
- [MSW Snyk Security Page](https://security.snyk.io/package/npm/msw)
- [@faker-js/faker npm](https://www.npmjs.com/package/@faker-js/faker)
- [tauri-remote-ui GitHub](https://github.com/draviavemal/tauri-remote-ui)
- [Storybook 8 vs Ladle comparison](https://www.pkgpulse.com/blog/storybook-8-vs-ladle-vs-histoire-component-development-2026)
- [DEV.to: Tauri v2 + React 19 desktop AI app (browser dev pattern)](https://dev.to/purpledoubled/how-i-built-a-desktop-ai-app-with-tauri-v2-react-19-in-2026-1g47)
- `docs/plans/dev-web-frontend/research-practices.md` (CrossHook codebase analysis)
