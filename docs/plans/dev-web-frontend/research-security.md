# Security Research: dev-web-frontend Mode

**Date**: 2026-04-07
**Scope**: Security evaluation of a browser-only dev mode for CrossHook's React/Vite frontend, using mock data to allow design iteration without the Tauri runtime.

---

## Executive Summary

The proposed dev-web-frontend feature is **implementable safely** with no CRITICAL blockers, provided the implementation follows the mitigations below. The primary risks are:

1. **Mock code leaking into production builds** — addressable through `import.meta.env.DEV` guards combined with a custom `MODE` check, but dynamic `import()` of mock modules requires a specific pattern to guarantee elimination.
2. **Dev server LAN exposure** — opt-in only (no default risk), but must be documented as a non-option in CI and developer onboarding.
3. **Fixture content sensitivity** — fixtures must use fully synthetic data; no copy-paste from real user profiles.
4. **New dependencies** — MSW (if adopted) has a postinstall Service Worker write; `@faker-js/faker` carries historical sabotage precedent. Both are manageable with standard controls.

No authentication/authorization surface exists. CrossHook is an offline desktop app with no user accounts. The Tauri IPC is not exposed in browser mode by design.

---

## Findings by Severity

### CRITICAL

**None identified** for correctly-implemented feature. The conditional below escalates to CRITICAL if violated.

> **Would become CRITICAL**: If mock adapter code is importable in a production build (i.e., the tree-shaking guard is absent or bypassed), the Tauri WebView would silently call mock data instead of real Rust IPC, masking real bugs and delivering a false sense of a working application. Preventing this is the single most important implementation requirement.

---

### WARNING

#### W-1: Dynamic `import()` of mock modules is NOT automatically tree-shaken

**Finding**: Vite 8 uses Rolldown/Oxc for production builds. Static `import.meta.env.DEV` guards are replaced with `if (false)` at compile time and the Oxc minifier removes the dead block. However, **dynamically imported modules (via `import()`)** have a structural pipeline limitation: Rollup tracks dynamic `import()` expressions for chunk graph construction _before_ dead code branches are fully pruned. Define substitution (plugin layer) runs before minification, but module resolution happens at chunk graph build time. The result: even inside a statically unreachable `if (false)` branch, `import('./mocks/index')` can cause Rollup to resolve and include that module as a chunk in `dist/`. The Oxc minifier removes the dead block from JS output, but the chunk may already have been emitted. This is a confirmed Vite limitation (issue #5676).

**Risky pattern** — eager promise evaluated at module initialization, before the guard:

```ts
// RISKY — import() resolved at module init; module included in chunk graph
const mockRegistryPromise = isWebDev ? import('./mocks/index').then((m) => m.mockRegistry) : Promise.resolve(null);
```

**Safe pattern** — use a `define` compile-time boolean constant and a branch-local dynamic import. Rollup sees `if (false) { import(...) }` as fully unreachable and drops the import node before building the chunk list:

```ts
// vite.config.ts
define: {
  __WEB_DEV_MODE__: JSON.stringify(mode === 'webdev'),
}

// lib/ipc.ts
declare const __WEB_DEV_MODE__: boolean;

export async function callCommand<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    return invoke<T>(name, args);
  }
  if (!__WEB_DEV_MODE__) {
    throw new Error(`callCommand('${name}') invoked outside Tauri in non-webdev mode`);
  }
  // Statically unreachable in production. @vite-ignore suppresses Rollup's
  // "can't analyze dynamic import" warning only — not bundling behavior.
  const { mockRegistry } = await import(/* @vite-ignore */ './mocks/index');
  const handler = mockRegistry.get(name);
  if (!handler) throw new Error(`no mock handler for '${name}'`);
  return handler(args ?? {}) as Promise<T>;
}
```

**Verification requirement (mandatory CI gate)**: Add an assertion step to `release.yml` immediately after the AppImage build step. The AppImage bundle output is in `src/crosshook-native/dist/assets/` (confirmed from `build-native.sh`'s `DIST_DIR`). Sentinel strings must be unambiguous — not generic terms that could appear in legitimate UI copy:

```bash
# In release.yml, after "Build native AppImage":
- name: Assert no mock code in production bundle
  run: |
    if grep -rl '\[dev-mock\]\|getMockRegistry\|mockIPC' \
        src/crosshook-native/dist/assets/*.js 2>/dev/null; then
      echo "ERROR: Mock code found in production bundle" >&2
      exit 1
    fi
    echo "Bundle clean — no mock strings found"
```

Use `[dev-mock]` (the error prefix in the missing-handler throw) and `getMockRegistry` as sentinels — both exist only in `lib/mocks/`. This is the authoritative production safety check; the `__WEB_DEV_MODE__` guard is the primary mechanism, this is the safety net.

---

#### W-2: Dev server host exposure on shared networks

**Finding**: `vite.config.ts` currently sets `host: host || false` where `host` is read from `TAURI_DEV_HOST`. This is safe for normal Tauri dev usage (host is unset, so server binds to `127.0.0.1` only). However, for the new `--dev` browser mode, a developer on a shared network (office, conference Wi-Fi) might pass `--host 0.0.0.0` to test on a phone or second machine. This exposes:

- All source files (including fixture paths that may reveal system layout)
- The HMR WebSocket to the LAN
- Any fixture data containing synthetic-but-believable paths

Additionally, a historical Vite security advisory (GHSA-vg6x-rcgg-rjx6) showed that even localhost-only dev servers were vulnerable to CORS-based source theft from a malicious web page visited during a dev session, in versions prior to Vite 6.0.9 / 5.4.12 / 4.5.6. CrossHook's `package.json` pins `"vite": "^8.0.5"` — this is patched.

**Mitigation**:

1. The `dev-native.sh --dev` script must **not** pass `--host` by default. Document explicitly that `--host 0.0.0.0` is not supported.
2. Add a comment in `vite.config.ts` for the webdev mode path noting that `server.host` must remain `false` (localhost only).
3. Confirm Vite 8.x default CORS behavior remains restricted to localhost/loopback — this should be verified when the vite config is extended for webdev mode.

---

#### W-3: Fixture files committed to repo may contain real system paths or user data

**Finding**: A common developer shortcut is to capture real app state to generate realistic fixtures. If any fixture JSON contains actual Steam library paths, real game names, actual profile names, or real installation paths, it leaks PII and system layout information into the public repository.

**Mitigation**:

1. **Policy (required for MVP)**: Fixtures must use fully synthetic data only. No copy-paste from `~/.local/share/crosshook/` or any real user state. Document the rule in a comment at the top of `lib/mocks/index.ts` and in `CONTRIBUTING.md`. Follow the same pattern as the existing codebase constants (`DEFAULT_APP_SETTINGS`, `createDefaultProfile()`) which already use empty strings and `false` for all path/ID fields.
2. **Naming (required for MVP)**: Use constants with obviously synthetic values (`MOCK_GAME_PATH = '/mock/game/game.exe'`, `MOCK_STEAM_ID = '0000000000001'`) so fixture content is visually recognizable as fake on inspection.
3. **CI grep (follow-up, not MVP-blocking)**: After the feature ships, add a grep check scoped to `lib/mocks/fixtures/` only. Two patterns cover distinct field types:
   - **SteamID64 / User IDs** (PII, flagged by security scanners): `\b[0-9]{17}\b`
   - **Steam App IDs** (misrepresentation risk): prefer positive enforcement — define `MOCK_APP_ID_BASE = 9_000_001` and require all fixture App IDs to exceed that constant; the grep (`\b[0-9]{7,10}\b`) is a backstop only, since the pattern also matches timestamps and port numbers
   - **Home directory paths**: grep for `/home/`, `/users/`, `/Users/` — but scope the check to string values in TS/JSON, not whole files, to avoid matching comments that use these patterns as examples

---

### ADVISORY

#### A-1: Mode check must be confined to `lib/ipc.ts` — not scattered across components

**Finding**: The original concern was that `import.meta.env.MODE === 'webdev'` string comparisons are less reliable for tree-shaking than a `define` compile-time boolean. After codebase review, the more important rule is architectural: **no component should ever check the mode at all**. If components check mode directly — whether via `import.meta.env.MODE`, `__WEB_DEV_MODE__`, or `isTauri()` — there are multiple sites where mock data can leak through a wrong condition. If components only call `callCommand()` and receive data, there is nothing to get wrong at the component level.

The `define` constant (`__WEB_DEV_MODE__`) retains one concrete advantage over `import.meta.env.MODE`: it is a static boolean that Rollup evaluates at bundle time before building the chunk graph, giving stronger dead-code elimination guarantees for the dynamic import in `lib/ipc.ts` (see W-1). `import.meta.env.MODE` is replaced with a string literal and requires the minifier to fold the string comparison — one extra step that is generally reliable in Oxc but is not Rollup-level static analysis.

**Mitigation**:

1. **Architectural rule (primary)**: Mode checks live only in `lib/ipc.ts`. No component, hook, or utility outside `lib/ipc.ts` checks `import.meta.env.MODE`, `__WEB_DEV_MODE__`, or `isTauri()` for the purpose of routing to mock data. Enforce via ESLint `no-restricted-imports` blocking `**/lib/mocks/**` from anywhere except `**/lib/ipc.ts`.
2. **Mechanism — two options with different tradeoffs**:

   | Mechanism                                        | Rollup elimination                                                  | Misconfiguration risk                                                                                                                                                                    |
   | ------------------------------------------------ | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
   | `__WEB_DEV_MODE__` via `vite.config.ts` `define` | Stronger — Rollup sees a literal `false`, folds before chunk graph  | Silent failure if `dev:browser` script omits `--mode webdev` — constant is `false` in plain `vite`, mock layer never activates, adapter calls real `invoke`, throws with no Tauri bridge |
   | `import.meta.env.MODE === 'webdev'`              | Weaker — string comparison folded by Oxc minifier after chunk graph | Lower — `MODE` is set automatically from `--mode`; no extra config required                                                                                                              |

   **If using `__WEB_DEV_MODE__`**: the `vite.config.ts` `define` block must be mode-conditional, and the `dev:browser` npm script **must** pass `--mode webdev` to Vite:

   ```ts
   // vite.config.ts
   export default defineConfig(({ mode }) => ({
     define: {
       __WEB_DEV_MODE__: mode === 'webdev',
     },
     // ... rest of config
   }));
   ```

   ```json
   // package.json
   "dev:browser": "vite --mode webdev"
   ```

   Omitting `--mode webdev` causes `__WEB_DEV_MODE__` to be `false` in browser dev mode — the mock layer silently never activates. The failure mode is confusing: the adapter calls real `invoke()`, which throws because no Tauri bridge exists, making it look like a Tauri error rather than a misconfigured script.

   **If using `import.meta.env.MODE`**: plain `vite --mode webdev` suffices with no extra `define` config. The `verify:no-mocks` CI gate remains required to cover the weaker chunk-graph guarantee.

   Either way, the `verify:no-mocks` CI gate is the authoritative production safety check — not the guard mechanism alone.

---

#### A-2: MSW postinstall script writes a Service Worker file to `public/`

**Finding**: MSW's `msw init` command (triggered by its postinstall hook when `msw.workerDirectory` is set in `package.json`) writes `mockServiceWorker.js` into the public directory. This file:

- Is a real Service Worker that intercepts network requests
- Should be committed to the repo (MSW recommends this)
- Must **not** be registered in production builds

If MSW is added to `devDependencies` and the worker registration is guarded by `import.meta.env.DEV`, the registration call is eliminated from production bundles. However, the `mockServiceWorker.js` file itself will exist in `public/` and will therefore be copied into `dist/` during every build.

**Risk level**: LOW for production behavior (the file is not registered, so it never intercepts), but it does bloat the production bundle and constitutes unnecessary surface area.

**Mitigation**:

1. Add `public/mockServiceWorker.js` to `.gitignore` if possible, or add a `vite build` postprocess step that deletes it from `dist/`.
2. Consider using `msw` without the browser Service Worker (Node.js/handler mode only) if the mock data is served entirely in-process without actual network-level interception.
3. Alternatively, consider a simpler adapter pattern (described in W-1's safe pattern) that does not require MSW at all for this use case — CrossHook is not intercepting HTTP requests; it is replacing Tauri IPC calls.

---

#### A-3: `@faker-js/faker` historical supply chain incident

**Finding**: The original `faker.js` package was sabotaged by its maintainer in January 2022. The community fork `@faker-js/faker` was created as a response and is now the canonical replacement with active governance. As of 2025-2026, `@faker-js/faker` has no known CVEs and is actively maintained. The Snyk advisor shows it is among the top 10,000 npm packages by download volume.

**Mitigation**:

1. If `@faker-js/faker` is adopted, pin to an exact version (`"@faker-js/faker": "9.x.x"`) rather than a caret range, to prevent silent upgrades that could introduce unexpected behavior.
2. Use `npm audit` and consider adding a Dependabot / Renovate configuration to alert on new versions before they are auto-applied.
3. If fixture data is static (a small set of hardcoded games, profiles), prefer hand-authored JSON fixtures over a faker runtime dependency — this eliminates the dependency entirely, reduces bundle scope, and makes fixtures reviewable at a glance.

---

#### A-4: `isTauri()` detection helper spoofability

**Finding**: Tauri v2's standard detection approach uses the probe `typeof window.__TAURI_INTERNALS__ !== 'undefined'`. This is the official Tauri v2 mechanism — set by the Tauri WebView bridge, not injectable from the page. In a normal browser, it is always absent.

CVE-2024-35222 (Tauri iFrame IPC bypass) is **not applicable** here: it required script execution inside an iFrame of a real Tauri app. Browser-mode dev sessions have no Tauri runtime at all.

A malicious page cannot inject `window.__TAURI_INTERNALS__` to spoof the check in a real Tauri session because:

1. The Tauri WebView's CSP (see below) blocks external script execution
2. The IPC is initialized by the Tauri core via its own injected scripts, not by the frontend

When `isTauri()` is centralized in a single `lib/runtime.ts` module and inlined as a boolean literal in production (`TAURI_ENV_DEBUG=0`) builds, Vite/oxc's dead-code elimination removes the entire mock branch — the mock registry, mock data, and `lib/mocks/` are not present in production AppImage builds. Scattering the probe across many files removes this auditability guarantee.

**However**: If the `isTauri()` helper is implemented as a simple truthy check without verifying the IPC key, a developer testing in a browser could accidentally simulate a Tauri environment by patching `window`. This is a development ergonomics risk, not a production security risk.

**Mitigation**: The detection helper is fine for this use case. Centralize it in one module (`lib/runtime.ts`). Document that it is a heuristic for dev routing, not a security boundary. The real security boundary is Tauri's capability system on the Rust side.

---

#### A-5: Production source maps must not include mock fixture content

**Finding**: `vite.config.ts` sets `sourcemap: isDebug` where `isDebug = !!process.env.TAURI_ENV_DEBUG`. In normal Tauri production builds, source maps are disabled because `TAURI_ENV_DEBUG` is unset. However, the webdev mode may be invoked entirely outside the Tauri build pipeline (e.g., `vite build --mode webdev` directly), in which case `TAURI_ENV_DEBUG` is also unset but the implicit assumption that "unset means no sourcemaps" still holds. The problem is that if someone adds `TAURI_ENV_DEBUG=1` for any reason during a webdev build, source maps are emitted containing the full `lib/mocks/**` source tree.

Additionally, the Oxc pipeline ordering guarantee (dead code eliminated before sourcemap emission) is not a specification guarantee. If a mock module is included in the chunk graph at all (see W-1), its source will appear in the sourcemap regardless of whether it's reachable.

**Mitigation**: The existing `sourcemap: isDebug` config already produces `sourcemap: false` for both webdev and production builds, since `TAURI_ENV_DEBUG` is only set by `tauri dev` and `tauri build --debug` — plain `npm run dev` (webdev mode) never sets it. This is not a current bug.

The risk is forward-looking: if a future `vite.config.webdev.ts` or `--mode webdev` override sets `sourcemap: true` for easier browser debugging, and that config is accidentally referenced from `build-native.sh` or the release workflow, mock source content would be exposed.

**Constraint for implementation**: Any webdev-specific Vite config overrides must explicitly set `sourcemap: false` and must not be referenced from release scripts. Document this as a constraint in the implementation plan, not a code change. The existing config line is already correct:

```ts
sourcemap: isDebug ? 'inline' : false,
```

No change needed for MVP.

---

#### A-6: Visual indicator requirement for dev mode screenshots

**Finding**: The `populated` fixture will contain realistic-looking fake data — game titles, profile configs, community entries. A screenshot shared publicly (GitHub issue, Slack, blog post) could be mistaken for real user data or real game library contents. The risk is **misrepresentation, not data exposure** — there is no connection to any real database, Steam library, or user account in dev mode. The fixture data is static TypeScript objects only.

Two specific failure modes:

1. A full-viewport screenshot that omits or crops the dev indicator chip
2. Fixture data that looks real enough that reviewers don't question it (e.g., real game names, plausible Steam App IDs)

**Mitigations — both required, defense in depth**:

1. **Two-layer visual indicator (non-dismissable, crop-resistant)**:
   - **Layer 1 — inset viewport outline**: `box-shadow: inset 0 0 0 3px var(--crosshook-color-warning)` applied to `.crosshook-app`. Uses `inset box-shadow` so it has zero layout impact — no height shift, no effect on scroll areas or height-sensitive components. Visible on all four edges; survives any reasonable screenshot crop. A top banner was explicitly rejected because it shifts vertical layout, breaking height-sensitive measurements that the feature exists to iterate on.
   - **Layer 2 — corner chip**: `position: fixed; bottom: 12px; right: 12px`, no close button, labels mode and fixture state in human-readable text.

   Both layers must render from the **layout root before any content** — not from a child component after fixture data loads. The confirmed implementation point is `App.tsx`, which owns `<main class="crosshook-app">` and sits above `<ProfileProvider>`. `AppShell` is inside `ProfileProvider` and is therefore not the correct placement:

   ```tsx
   // App.tsx — confirmed implementation
   declare const __WEB_DEV_MODE__: boolean;

   <main className={`crosshook-app crosshook-focus-scope${__WEB_DEV_MODE__ ? ' crosshook-app--webdev' : ''}`}>
     {__WEB_DEV_MODE__ && <DevModeChip />}
     <ProfileProvider>...</ProfileProvider>
   </main>;
   ```

   The CSS for `.crosshook-app--webdev` lives in a dedicated `dev-indicator.css` file imported only in the webdev entry point — it is excluded from the production stylesheet entirely. The class itself is harmless in production (no element will carry it), but keeping it out of the production CSS is cleaner.

   **Condition requirement**: `__WEB_DEV_MODE__` exclusively — no `VITE_*` env variables. `VITE_*` vars are inlined at build time from the environment; a misconfigured CI running `VITE_WEB_DEV=true vite build` would ship the indicator class in the production bundle. `__WEB_DEV_MODE__` is set explicitly to `false` by `vite.config.ts` for all non-webdev builds, cannot be overridden by the environment, and is the single activation point for the entire webdev feature.

2. **Obviously fake fixture data (see also W-3)**: Game titles must use placeholder names (`Test Game Alpha`, `Dev Profile 1`), not real titles. No real Steam App IDs — use values outside the valid range (e.g., `9999001`+) or clearly fictional values. Community entries must use placeholder text. This is a **mandatory content policy**, not a suggestion — it is the only mitigation that survives a cropped screenshot.

The watermark pattern (diagonal "PREVIEW" overlay) is not needed and would interfere with design review. Do not implement it.

---

## Dependency Security

**Zero new dependencies are introduced by this feature.** The implementation is ~70 lines of hand-rolled TypeScript across `lib/runtime.ts`, `lib/ipc.ts`, and `lib/mocks/index.ts`. No `npm audit` or transitive dependency review is required for this feature.

The three libraries evaluated during research were considered and rejected:

| Library                 | Reason not adopted                                                                                                                                                                                   |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@tauri-apps/api/mocks` | Requires `window.__TAURI_INTERNALS__` (the Tauri bridge) to be present — it does not exist in a pure browser session. Also designed for test-runner teardown patterns, not a persistent dev session. |
| `msw`                   | Intercepts HTTP `fetch`/`XHR`. Tauri `invoke()` is a WebView bridge call, not HTTP — MSW cannot intercept it. Wrong layer for this use case.                                                         |
| `@faker-js/faker`       | Not needed for initial feature. Existing codebase patterns (`DEFAULT_APP_SETTINGS`, `createDefaultProfile()`) plus hand-authored static fixture objects are sufficient.                              |

### Existing Dependencies (No Change)

No existing dependencies need to be modified for this feature. The mock adapter pattern is additive and isolated. If `@faker-js/faker` or `msw` are reconsidered in future iterations, the supply chain assessments documented during research remain on record: `@faker-js/faker` v10 has no known CVEs but carries sabotage history requiring exact-version pinning; `msw` v2 has no direct CVEs but `tough-cookie` and `path-to-regexp` transitive deps require `npm audit` before adoption.

---

## Authentication and Authorization

**N/A.** CrossHook is an offline-only desktop application. There are no user accounts, no session tokens, no auth flows, and no network endpoints. The Tauri capability system (defined in `src/crosshook-native/src-tauri/tauri.conf.json`) gates which Tauri APIs the frontend can call — this is orthogonal to the mock adapter and unaffected by the dev-web-frontend feature.

---

## Data Protection

1. **Fixture data**: Must be synthetic. See W-3 and A-3 for specifics.
2. **No network transmission**: The dev server binds to localhost only. No user data leaves the machine.
3. **No persistence**: The mock adapter should not write to disk. Any state should be in-memory and reset on page reload.
4. **No secrets in fixtures**: Fixtures must not contain API keys, tokens, or credentials, even fake-looking ones (they can end up in grep results and confuse security scanners).
5. **Fixtures must live in `lib/mocks/fixtures/`**: Not in `public/`. Files in `public/` are served as static assets by the Vite dev server and are directly fetchable at `http://localhost:5173/fixtures/...`. Files in `lib/mocks/` are only reachable via the JS module graph.
6. **No `VITE_*` env vars in mock handlers**: `envPrefix: ['VITE_', 'TAURI_ENV_*']` in `vite.config.ts` means any `VITE_*` variable is inlined into the bundle. If a mock handler reads `import.meta.env.VITE_STEAMGRIDDB_KEY`, that key's value is embedded in the bundle for any build that includes the mock module. Mock handlers must never read `import.meta.env.*` outside of `MODE` and `DEV`. For dev-only configuration of mock behavior (e.g., simulating error states), use URL hash params or manually-set `localStorage` flags — these leave no trace in the bundle.

---

## Input Validation

The mock adapter receives no user input — it returns static or factory-generated data. No input validation is required in the mock layer itself. The real Rust-side validation continues to exist in `crosshook-core` and is tested independently via `cargo test`.

---

## Infrastructure Security

**Current CSP** (from `tauri.conf.json`):

```
default-src 'self'; script-src 'self'; img-src 'self' asset: http://asset.localhost
```

This CSP applies inside the Tauri WebView. It does **not** apply to the browser dev session (Vite's dev server serves the app in Chrome/Firefox, which has its own security model).

**For browser mode specifically**:

- The Vite dev server does not inject a CSP by default.
- Since this is a developer-only tool running on localhost, the absence of a strict CSP is acceptable.
- However, if a developer inadvertently loads the dev server URL in a browser tab while also browsing the web, the existing Vite CORS protections (in Vite 8.x) prevent cross-origin JS from reading dev server content.

**Recommendation**: No CSP configuration is required for the browser dev mode. The default Vite 8.x localhost-only CORS posture is sufficient.

---

## Secure Coding Guidelines

The following rules must be enforced during implementation:

1. **Single guard point**: The mock adapter must be activated in exactly one place — the adapter factory. No other code should branch on `isTauri()` or `__WEB_DEV_MODE__` to load mock vs. real data.
2. **No conditional imports at component level**: Components must not import mock utilities directly. All mock data must flow through the adapter interface, which is identical to the Tauri adapter interface.
3. **TypeScript types must be shared**: Both `TauriAdapter` and `MockAdapter` must implement the same interface type. This ensures type errors surface immediately if the mock diverges from real IPC signatures.
4. **No `any` types in mock data**: Fixture typing must match the exact types used in production IPC responses. Loose fixture types mask type regressions.
5. **No `console.log` in mock adapter**: Use the existing logging infrastructure or omit logs. `console.log` in the mock path could appear in production if the guard fails.
6. **Mock adapter must not write to disk**: No `fs` calls, no Tauri plugin usage. The mock adapter is memory-only.
7. **Plugin stubs must throw visibly on destructive operations**: Stubs for `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-shell`, and `@tauri-apps/plugin-fs` expose which native capabilities the app uses. Stubs must **not** silently succeed (e.g., returning `undefined`) on operations that would be destructive or data-mutating in production (file save, shell exec, destructive dialog confirm). Instead, they should `throw new Error('plugin-fs/writeFile is not available in browser dev mode')`. Silent no-ops mask bugs where production code depends on a side effect that never fires.
8. **`listen()` stubs must return a real cleanup function**: Event listener stubs (`listen`, `once`) must return a resolved `Promise<() => void>` with a no-op unsubscribe function. Hooks that rely on the unlisten return value to deregister listeners will otherwise accumulate unresolvable teardown calls, creating memory leak patterns in long-running dev sessions.

---

## Trade-off Recommendations

| Decision                         | Recommended choice                                            | Rationale                                                                                                                                                                                   |
| -------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mock activation guard            | `define: { __WEB_DEV_MODE__: boolean }` compile-time constant | More reliable than `import.meta.env.MODE` string comparison; guaranteed `false` in all non-webdev builds                                                                                    |
| Dynamic vs static mock import    | Static import with dead-branch pruning                        | Dynamic `import()` of mock modules has known Vite/Rollup tree-shaking limitations                                                                                                           |
| MSW vs adapter pattern           | Adapter pattern only (no MSW)                                 | CrossHook mocks Tauri IPC, not HTTP. MSW adds a Service Worker that must be managed in `public/`. An adapter interface is simpler, has no postinstall side effects, and is zero-dependency. |
| `@faker-js/faker` vs static JSON | Static JSON fixtures                                          | For a bounded fixture set (a dozen games, a few profiles), hand-authored JSON is reviewable, has no dependency risk, and is faster at runtime                                               |
| Fixture content                  | Fully synthetic, obviously fake                               | Prevents PII leakage; makes it clear to all reviewers that data is not real                                                                                                                 |
| Source maps in webdev mode       | Disabled (same as production)                                 | Prevents accidental source/fixture exposure in shareable builds                                                                                                                             |

---

## Open Questions

1. **Bundle verification**: Who runs the bundle size / content check after each release? Should this be a CI step that greps `dist/` for known mock-only strings (e.g., `MockAdapter`, fixture identifiers)?
2. **Vite 8 CORS defaults**: Vite 8 is relatively new. The team should verify that `server.cors` defaults in Vite 8.x remain localhost-only (inherited from the fix in 5.4.12/6.0.9). Check `vite/packages/vite/src/node/server/index.ts` in the Vite 8 source or Vite 8 changelog.
3. **mockServiceWorker.js in .gitignore**: If MSW is ultimately adopted, does the team want to commit `mockServiceWorker.js` (MSW recommends this) or exclude it and regenerate on setup? Decision needed before implementation.
4. **Dev mode indicator**: UX team to define the visual "MOCK DATA" indicator; security requires it be non-dismissable and visually distinct from any element that appears in production screenshots.

---

## Sources

- [Vite: Env Variables and Modes](https://vite.dev/guide/env-and-mode) — `import.meta.env.DEV` tree-shaking behavior
- [Vite GitHub Issue #11080: Dynamic imports are not tree-shaken](https://github.com/vitejs/vite/issues/11080) — known limitation
- [Vite GitHub Issue #15256: Tree-shaking with environment variables](https://github.com/vitejs/vite/issues/15256) — undefined variable edge case
- [Vite Security Advisory GHSA-vg6x-rcgg-rjx6](https://github.com/vitejs/vite/security/advisories/GHSA-vg6x-rcgg-rjx6) — CORS/WebSocket source theft (patched in Vite ≥6.0.9)
- [MSW npm package](https://www.npmjs.com/package/msw) — maintenance status, postinstall behavior
- [MSW: Managing the worker](https://mswjs.io/docs/best-practices/managing-the-worker/) — worker script lifecycle
- [Snyk: @faker-js/faker](https://security.snyk.io/package/npm/@faker-js/faker) — vulnerability scan
- [Bleeping Computer: Dev corrupts faker.js](https://www.bleepingcomputer.com/news/security/dev-corrupts-npm-libs-colors-and-faker-breaking-thousands-of-apps/) — supply chain sabotage history
- [CVE-2024-35222: Tauri iFrame IPC bypass](https://github.com/tauri-apps/tauri/security/advisories/GHSA-57fm-592m-34r7) — iFrame origin bypass (not applicable to this feature)
- [Tauri v2 Security docs](https://v2.tauri.app/security/) — capability system and CSP reference
- [OWASP NPM Security Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/NPM_Security_Cheat_Sheet.html) — supply chain controls
