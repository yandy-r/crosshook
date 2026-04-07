# dev-web-frontend Implementation Plan

This plan ships a browser-only Vite dev mode for CrossHook's React frontend by introducing an owned IPC adapter layer (`runtime.ts`, `ipc.ts`, `events.ts`, `plugin-stubs/`, `mocks/`) under `src/crosshook-native/src/lib/`, a `./scripts/dev-native.sh --browser` flag, and a CI grep sentinel that guarantees no mock code reaches the production AppImage. Every `invoke()` (42 files) and `listen()` (13 files) call site migrates mechanically to `callCommand()` / `subscribeEvent()`, three plugin packages get local stubs, and a two-layer dev-mode indicator (inset amber outline + corner chip) makes browser mode unmistakable. The work is partitioned into Phase 1 (single PR — 18 tasks across 5 waves with 13 of 18 parallelizable), Phase 2 (13 handler-fanout sub-PRs, fully parallel), and Phase 3 (7 polish sub-PRs, fully parallel). Zero new npm dependencies, runtime-only state, no persistence boundary changes — Phase 1 is structurally feature-complete and unlocks the entire Phase 2 fan-out without further coordination.

## Critically Relevant Files and Documentation

- docs/plans/dev-web-frontend/feature-spec.md: 819-line pre-synthesized feature specification — single source of truth; resolved decisions D1-D6 are settled
- docs/plans/dev-web-frontend/shared.md: Synthesized overview with file paths, patterns, and must-read docs
- docs/plans/dev-web-frontend/analysis-context.md: Condensed planning context with cross-cutting concerns and parallelization guidance
- docs/plans/dev-web-frontend/analysis-code.md: Live-code analysis with full file lists (42 invoke + 13 listen + 6 plugin) and per-pattern examples
- docs/plans/dev-web-frontend/analysis-tasks.md: Dependency graph and task structure for Phases 1-3
- docs/plans/dev-web-frontend/research-technical.md: Per-module implementation snippets, `convertFileSrc` synchrony edge case
- docs/plans/dev-web-frontend/research-security.md: 3 critical findings + 3 warnings + 6 advisories with required mitigations
- docs/plans/dev-web-frontend/research-business.md: BR-1 through BR-12 business rules + IPC call inventory
- docs/plans/dev-web-frontend/research-ux.md: Two-layer dev indicator spec + accessibility requirements
- src/crosshook-native/src/App.tsx: Root shell; line 5 imports `listen`, line 67 subscribes to `onboarding-check`; needs migration + chip wire
- src/crosshook-native/src/main.tsx: React entry; eager-import point for `lib/plugin-stubs/convertFileSrc`
- src/crosshook-native/vite.config.ts: Currently flat `defineConfig({...})`; must convert to `defineConfig(({ mode }) => ({...}))` for mode-conditional define
- src/crosshook-native/package.json: Add `dev:browser` script
- src/crosshook-native/tsconfig.json: Add `paths` alias matching Vite alias
- src/crosshook-native/src/vite-env.d.ts: Add `__WEB_DEV_MODE__` declaration
- src/crosshook-native/src/context/PreferencesContext.tsx: Most critical migration target — parallel 3-command boot at lines 43-46
- src/crosshook-native/src/hooks/useLibrarySummaries.ts: Local `ProfileSummary` interface (lines 6-12) must move to `types/library.ts`
- src/crosshook-native/src/types/library.ts: Target for `ProfileSummary` export; not currently in `types/index.ts` barrel
- src/crosshook-native/src/types/index.ts: Add missing `export * from './library'`
- src/crosshook-native/src/types/settings.ts: `DEFAULT_APP_SETTINGS`, `toSettingsSaveRequest` reused by handlers
- src/crosshook-native/src/types/profile.ts: `createDefaultProfile`, `normalizeSerializedGameProfile` reused by handlers
- src/crosshook-native/src/utils/optimization-catalog.ts: Non-hook invoke site
- src/crosshook-native/src/styles/variables.css: `--crosshook-color-warning` token for chip + outline
- scripts/dev-native.sh: Add `--browser`/`--web` case branch
- .github/workflows/release.yml: Add `verify:no-mocks` step after AppImage build
- AGENTS.md: Commands reference block update
- CLAUDE.md: `docs(internal):` commit prefix policy + persistence boundary rules

## Implementation Plan

### Phase 1: Foundation (single PR per D2)

Foundation phase ships the adapter layer, plugin stubs, mock skeleton, boot-critical handlers, mechanical migration, dev-mode indicator, and CI safety gate in one coherent PR. The declared dependencies yield a **6-wave critical path**: Wave 1 (5 independent foundation tasks: 1.1–1.5) → Wave 2 (7 parallel adapter/stub/indicator tasks: 1.6, 1.7, 1.8, 1.9, 1.10, 1.11, 1.15) → Wave 3 (mock skeleton 1.12, which depends on 1.7 from Wave 2) → Wave 4 (2 parallel boot handler tasks: 1.13, 1.14) → Wave 5 (1 critical-path mechanical migration: 1.16) → Wave 6 (2 parallel leaf tasks: 1.17, 1.18). 13 of 18 tasks have no intra-wave blocking constraint; the critical-path length is 6.

#### Task 1.1: Vite mode-conditional config + path alias + webdev declarations Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/vite.config.ts
- src/crosshook-native/tsconfig.json
- src/crosshook-native/src/vite-env.d.ts
- docs/plans/dev-web-frontend/feature-spec.md
- docs/plans/dev-web-frontend/research-security.md

**Instructions**

Files to Modify

- src/crosshook-native/vite.config.ts
- src/crosshook-native/tsconfig.json
- src/crosshook-native/src/vite-env.d.ts

Convert `vite.config.ts` from the current flat `defineConfig({ ... })` form to a mode-aware function form `defineConfig(({ mode }) => ({ ... }))`. Add `define: { __WEB_DEV_MODE__: mode === 'webdev' }` so the constant is `true` only under `vite --mode webdev` and `false` for every other mode (including production `vite build` for the Tauri AppImage). Add `resolve: { alias: { '@': path.resolve(__dirname, './src') } }` so subsequent tasks can import via `@/lib/ipc`, `@/lib/events`, `@/lib/plugin-stubs/*`. For the `server` block, preserve the existing Tauri-mode behavior (`host: host || false` driven by `TAURI_DEV_HOST`) when `mode !== 'webdev'`, but force `host: '127.0.0.1'` and `strictPort: true` when `mode === 'webdev'` — this satisfies security finding W-2 and BR-9 (loopback only, no `--host 0.0.0.0` override). Add a `// security: webdev mode binds loopback only` comment explaining the hardcoding.

In `tsconfig.json`, add `"paths": { "@/*": ["./src/*"] }` to `compilerOptions` so TypeScript resolves the new alias in lockstep with Vite — without this, every `@/lib/*` import will fail TypeScript even if Vite resolves correctly. Verify `baseUrl` is `"./"` or `"./src"` (whichever the existing config uses) so the path mapping is well-defined.

In `vite-env.d.ts`, add `declare const __WEB_DEV_MODE__: boolean;` below the existing `ImportMeta` declarations. This makes the `define` constant visible to TypeScript globally, so `lib/ipc.ts` and `lib/mocks/index.ts` can reference it without any extra import.

Do NOT change anything in the `build` block — `target`, `minify`, `sourcemap` settings stay identical for both Tauri and webdev modes (per security advisory A-5, sourcemaps must remain disabled in webdev mode and the existing config achieves that since `isDebug = !!process.env.TAURI_ENV_DEBUG` is unset under plain `vite`).

#### Task 1.2: package.json dev:browser script Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/package.json
- docs/plans/dev-web-frontend/feature-spec.md

**Instructions**

Files to Modify

- src/crosshook-native/package.json

Add `"dev:browser": "vite --mode webdev"` to the `scripts` block, alongside the existing `dev`, `build`, `preview`, `tauri` entries. The `--mode webdev` flag is **mandatory** — it is the only way `__WEB_DEV_MODE__` evaluates to `true` per Task 1.1's `define` config. Plain `vite` defaults to `mode = 'development'`, which makes `__WEB_DEV_MODE__ = false`, which makes `ensureMocks()` (Task 1.6) throw a confusing internal error. Hard-coding the flag in the npm script is the primary footgun mitigation; the `ensureMocks()` runtime assertion is the secondary defense. Do not add any other scripts in this task — `dev:browser:check` (the handler-coverage script) is Phase 3 scope.

#### Task 1.3: dev-native.sh --browser case branch Depends on [none]

**READ THESE BEFORE TASK**

- scripts/dev-native.sh
- docs/plans/dev-web-frontend/feature-spec.md

**Instructions**

Files to Modify

- scripts/dev-native.sh

Add a `--browser|--web)` case branch to the existing `case "${1:-}" in` block (currently at lines 16-28). The branch must `cd "$NATIVE_DIR"`, ensure `node_modules/.bin/vite` exists (run `npm ci` if not), then `exec npm run dev:browser`. **Do NOT invoke `cargo`, `tauri`, or any native binary in this branch** — the entire purpose of `--browser` mode is to enable contributors without a Rust toolchain. The branch must come BEFORE the existing `--help|-h)` and `"")` (empty/Tauri) branches in case order so it matches first.

Update the `usage()` heredoc at lines 7-14 to document both `--browser` and `--web` as equivalent flags, with explicit notes:

- "Browser-only dev mode: starts Vite at <http://localhost:5173> with mock IPC"
- "Does not require cargo or the Rust toolchain"
- "Loopback only (--host 0.0.0.0 unsupported per security policy)"
- "Real Tauri behavior must be re-verified with `./scripts/dev-native.sh` before merge"

The script should work identically to before for the empty-arg and `--help` paths. The existing Wayland → X11 fallback logic (lines 49-54) only applies to the Tauri path and must not be triggered for `--browser`.

#### Task 1.4: lib/runtime.ts isTauri probe Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/dev-web-frontend/feature-spec.md
- docs/plans/dev-web-frontend/research-technical.md
- docs/plans/dev-web-frontend/analysis-code.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/runtime.ts

Create a single-purpose runtime probe module. Export `function isTauri(): boolean` that checks `'__TAURI_INTERNALS__' in window` (the Tauri v2 WebView bridge sets this property at startup). Wrap the access in a `typeof window !== 'undefined'` guard so the function is safe to call from any module-init context, including SSR-shaped tests. The function must be **zero-dep**: no React, no DOM beyond `window`, no `@tauri-apps/*` imports. This makes it the single source of truth that every other adapter module imports from, and prevents circular imports between `lib/ipc.ts` and `lib/events.ts`.

Add a one-line JSDoc comment explaining the purpose: `/** Returns true when running inside a Tauri v2 WebView, false in any plain browser context. Single source of truth for runtime branching across the lib/ adapter layer. */`. Do NOT use the official `isTauri()` from `@tauri-apps/api/core` — it depends on the runtime bundle and would import `@tauri-apps/api` in the browser path, defeating tree-shaking.

#### Task 1.5: Export ProfileSummary type from types/library.ts Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/library.ts
- src/crosshook-native/src/types/index.ts
- src/crosshook-native/src/hooks/useLibrarySummaries.ts
- docs/plans/dev-web-frontend/analysis-code.md

**Instructions**

Files to Modify

- src/crosshook-native/src/types/library.ts
- src/crosshook-native/src/types/index.ts
- src/crosshook-native/src/hooks/useLibrarySummaries.ts

Add `export interface ProfileSummary { name: string; gameName: string; steamAppId: string; customCoverArtPath?: string; customPortraitArtPath?: string; }` to `src/crosshook-native/src/types/library.ts`, alongside the existing `LibraryViewMode` and `LibraryCardData` exports. The shape is taken verbatim from the local interface currently at `useLibrarySummaries.ts` lines 6-12 — do not invent new fields.

In `src/crosshook-native/src/types/index.ts`, add `export * from './library';` to the barrel. The existing barrel re-exports 22 type modules but **omits `library.ts`** — this is a pre-existing gap that must be fixed so mock handlers (Task 1.14) can import `ProfileSummary` via `'../../../types'` rather than needing the longer relative path.

In `src/crosshook-native/src/hooks/useLibrarySummaries.ts`, remove the local `interface ProfileSummary` block (lines 6-12) and add `import type { ProfileSummary } from '../types/library';` at the top. The rest of the hook is unchanged — still calls `invoke<ProfileSummary[]>` (the migration to `callCommand` happens in Task 1.16). This task is a prerequisite for Task 1.14 (profile mock handler) which needs the typed export, but it has no upstream dependency itself — assign to wave 1 even though it modifies existing files.

#### Task 1.6: lib/ipc.ts callCommand adapter Depends on [1.1, 1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/runtime.ts
- src/crosshook-native/vite.config.ts
- docs/plans/dev-web-frontend/feature-spec.md
- docs/plans/dev-web-frontend/research-technical.md
- docs/plans/dev-web-frontend/research-security.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/ipc.ts

Implement the `callCommand<T>` IPC adapter following the spec at feature-spec.md lines 261-297. Key requirements:

1. Import `InvokeArgs` (type-only) from `@tauri-apps/api/core` and `isTauri` from `./runtime`. Do NOT import `invoke` at the top level — it must only enter the bundle inside the Tauri branch via dynamic import.
2. Declare the build-time constant: `declare const __WEB_DEV_MODE__: boolean;` (already declared globally by Task 1.1 but redeclare for clarity within this module).
3. Define `type Handler = (args: unknown) => unknown | Promise<unknown>;` and a module-scope `let mocksPromise: Promise<Map<string, Handler>> | null = null;` — the **promise latch**. This is critical because `PreferencesContext` fires 3 parallel `callCommand` calls on mount; without the latch they would each initiate a concurrent `import('./mocks')`, causing race conditions in `registerMocks()`. The latch ensures only the first call initiates the import and subsequent calls reuse the same promise.
4. Implement `async function ensureMocks(): Promise<Map<string, Handler>>` that:
   - Returns `mocksPromise` if already initialized
   - Throws `new Error('[dev-mock] mock layer invoked in non-webdev build — check dev:browser script passes --mode webdev')` if `!__WEB_DEV_MODE__` (this is the misconfiguration safety net required by D3)
   - Otherwise sets `mocksPromise = import(/* @vite-ignore */ './mocks').then(m => m.registerMocks())` and returns it
5. Export `async function callCommand<T>(name: string, args?: InvokeArgs): Promise<T>` that:
   - If `isTauri()`, dynamic-imports `@tauri-apps/api/core` and forwards to `invoke<T>(name, args)`
   - Otherwise calls `ensureMocks()`, looks up the handler by name, throws `new Error('[dev-mock] Unhandled command: ${name}. Add a handler in src/lib/mocks/handlers/<area>.ts — see lib/mocks/README.md')` if missing, otherwise invokes the handler and returns its result cast to `Promise<T>`
   - Logs `console.debug('[mock] callCommand', name, args)` only when `import.meta.env.DEV` (visible to contributors, stripped from production)

The dynamic `import('./mocks')` inside the dead branch is what allows Rollup to eliminate the entire `lib/mocks/` subtree from the production chunk graph when `__WEB_DEV_MODE__ = false`. This is the primary control of the three-layer security defense; the CI grep sentinel (Task 1.17) is the authoritative fail-safe.

#### Task 1.7: lib/events.ts subscribeEvent + emitMockEvent Depends on [1.1, 1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/runtime.ts
- src/crosshook-native/src/App.tsx
- src/crosshook-native/src/context/ProfileContext.tsx
- docs/plans/dev-web-frontend/feature-spec.md
- docs/plans/dev-web-frontend/analysis-code.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/events.ts

Implement the event-bus adapter following feature-spec.md lines 301-326. Key requirements:

1. Import `EventCallback`, `UnlistenFn` (type-only) from `@tauri-apps/api/event` and `isTauri` from `./runtime`. Do NOT import `listen` at the top level — same dynamic-import pattern as `ipc.ts`.
2. Define `type Listener = (payload: unknown) => void;` and a module-scope `const browserBus = new Map<string, Set<Listener>>();` — the in-process pub/sub bus.
3. Export `async function subscribeEvent<T>(name: string, handler: EventCallback<T>): Promise<UnlistenFn>` that:
   - If `isTauri()`, dynamic-imports `@tauri-apps/api/event` and forwards to `listen<T>(name, handler)`
   - Otherwise wraps the handler as `(payload) => handler({ event: name, id: 0, payload: payload as T })` (matching the Tauri Event<T> shape exactly), inserts it into the bus, and returns an unsubscribe function `() => { browserBus.get(name)?.delete(wrapped); }` that satisfies the `UnlistenFn = () => void` type.
4. Export `function emitMockEvent(name: string, payload: unknown): void` that no-ops in Tauri mode (so handler files can call it unconditionally) and otherwise iterates the bus listeners for that event name and invokes each. Phase 2 mock handlers will call this from inside mutating operations to drive realistic state transitions (e.g., `launch_game` triggers `launch-log` events).

The shape of the returned `UnlistenFn` is critical — the existing `listen()` cleanup pattern across 13 files uses `.then((unlisten) => unlisten())` or `.then((f) => f())`, and the adapter must match exactly so no cleanup code needs to change. BR-7 requires this be a real working unsubscribe — accumulating unresolvable teardown calls would create memory leaks in long-running dev sessions.

#### Task 1.8: lib/plugin-stubs/dialog.ts Depends on [1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/runtime.ts
- src/crosshook-native/src/utils/dialog.ts
- src/crosshook-native/src/components/CommunityBrowser.tsx
- docs/plans/dev-web-frontend/feature-spec.md
- docs/plans/dev-web-frontend/research-security.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/plugin-stubs/dialog.ts

Implement the dialog plugin stub. Re-export the real `@tauri-apps/plugin-dialog` API (`open`, `save`, `OpenDialogOptions`, `SaveDialogOptions`) when `isTauri()` is true via dynamic import. In browser mode, both `open()` and `save()` must:

- Return `Promise.resolve(null)` (mimicking user cancellation)
- `console.warn('[dev-mock] dialog suppressed in browser mode — call ignored')` so contributors notice the no-op
- (Optional) If a toast utility is easily reachable, surface a non-blocking toast; otherwise the warn is sufficient for Phase 1

Do NOT silently no-op — the warn is required by D4. Match the type signatures exactly so the existing `utils/dialog.ts` wrappers (`chooseFile`, `chooseSaveFile`, `chooseDirectory`) continue to compile without changes. The wrapper functions in `utils/dialog.ts` already handle null returns (treating them as cancel), so the stub's `null` return cleanly slots into existing logic.

Module structure:

```ts
import { isTauri } from '../runtime';
import type { OpenDialogOptions, SaveDialogOptions } from '@tauri-apps/plugin-dialog';

export type { OpenDialogOptions, SaveDialogOptions };

export async function open(options?: OpenDialogOptions): Promise<string | string[] | null> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-dialog');
    return real.open(options);
  }
  console.warn('[dev-mock] dialog.open suppressed in browser mode');
  return null;
}

export async function save(options?: SaveDialogOptions): Promise<string | null> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-dialog');
    return real.save(options);
  }
  console.warn('[dev-mock] dialog.save suppressed in browser mode');
  return null;
}
```

#### Task 1.9: lib/plugin-stubs/shell.ts Depends on [1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/runtime.ts
- src/crosshook-native/src/components/ExternalResultsSection.tsx
- src/crosshook-native/src/components/SettingsPanel.tsx
- docs/plans/dev-web-frontend/feature-spec.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/plugin-stubs/shell.ts

Implement the shell plugin stub. The CrossHook codebase only uses `@tauri-apps/plugin-shell.open(url)` (4 sites — opens a URL in the system browser). Re-export `open` from the real plugin in Tauri mode via dynamic import. In browser mode, `open(url)` should `console.warn('[dev-mock] shell.open suppressed in browser mode: ' + url)` and resolve to `undefined` — non-destructive operations may no-op safely with a loud warn per D4.

Also export a stub `Command` class (or function) where `Command.spawn()` and `Command.execute()` **throw** `new Error('[dev-mock] shell.Command.execute is not available in browser dev mode')` per D4. CrossHook does not currently call these in `src/`, but the type must exist so any future Phase 2 import does not silently break — and the throw is loud enough for contributors to spot the issue immediately.

Module structure:

```ts
import { isTauri } from '../runtime';

export async function open(url: string): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-shell');
    return real.open(url);
  }
  console.warn('[dev-mock] shell.open suppressed in browser mode:', url);
}

export class Command {
  static create(): Command {
    throw new Error('[dev-mock] shell.Command is not available in browser dev mode');
  }
  spawn(): never {
    throw new Error('[dev-mock] shell.Command.spawn is not available in browser dev mode');
  }
  execute(): never {
    throw new Error('[dev-mock] shell.Command.execute is not available in browser dev mode');
  }
}
```

#### Task 1.10: lib/plugin-stubs/fs.ts Depends on [1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/runtime.ts
- docs/plans/dev-web-frontend/feature-spec.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/plugin-stubs/fs.ts

Implement the fs plugin stub. CrossHook does not currently import `@tauri-apps/plugin-fs` directly from `src/` (the package is in `package.json` but only used transitively or by future code). The stub must still exist for completeness per D4 so any Phase 2 handler that imports it gets the right semantics.

Read operations (`readFile`, `readTextFile`, `exists`, `metadata`) return resolving stubs with synthetic data — empty strings, empty Uint8Arrays, `false`, or sensible default metadata. Write/destroy operations (`writeFile`, `writeTextFile`, `removeFile`, `removeDir`, `rename`, `createDir`, `mkdir`) **throw** `new Error('[dev-mock] fs.${op} is not available in browser dev mode')` per D4 — silent no-ops are banned because they would mask real bugs in Phase 2 install/update flows.

Module structure (illustrative — match the real plugin's named exports as closely as possible):

```ts
import { isTauri } from '../runtime';

export async function readTextFile(path: string): Promise<string> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.readTextFile(path);
  }
  console.warn('[dev-mock] fs.readTextFile returning empty string for:', path);
  return '';
}

export async function writeTextFile(path: string, contents: string): Promise<void> {
  if (isTauri()) {
    const real = await import('@tauri-apps/plugin-fs');
    return real.writeTextFile(path, contents);
  }
  throw new Error('[dev-mock] fs.writeTextFile is not available in browser dev mode');
}

// Repeat for readFile, writeFile, exists, metadata, removeFile, removeDir, rename, createDir, mkdir
```

#### Task 1.11: lib/plugin-stubs/convertFileSrc.ts Depends on [1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/runtime.ts
- src/crosshook-native/src/hooks/useGameCoverArt.ts
- src/crosshook-native/src/components/profile-sections/MediaSection.tsx
- docs/plans/dev-web-frontend/feature-spec.md
- docs/plans/dev-web-frontend/research-technical.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts

Implement a synchronous `convertFileSrc` shim. Unlike `invoke` and `listen`, `convertFileSrc` is a **synchronous** function called inside `useMemo` and event handlers in `useGameCoverArt.ts` and `MediaSection.tsx` — it cannot be wrapped in a `Promise` or dynamic `import()`. The static export must be available at module evaluation time, before any component renders.

Two strategies are valid:

**Strategy A (preferred)**: Conditionally re-export based on `isTauri()` evaluated at module init:

```ts
import { isTauri } from '../runtime';

let realConvertFileSrc: ((path: string, protocol?: string) => string) | null = null;
if (isTauri()) {
  // Synchronous import via top-level statement; only evaluated in Tauri mode
  // because of static analysis on `if (isTauri())`.
  // Note: this WILL include @tauri-apps/api/core in the bundle even in browser
  // mode unless we use the dynamic-import variant. See Strategy B if that's a concern.
  // For convertFileSrc specifically, the entire @tauri-apps/api/core surface
  // is small enough that the cost is acceptable.
}

export function convertFileSrc(path: string, protocol = 'asset'): string {
  if (isTauri() && realConvertFileSrc) {
    return realConvertFileSrc(path, protocol);
  }
  return path; // passthrough — caller's <img src> will fall back to placeholder
}
```

**Strategy B (cleaner tree-shaking)**: Statically import `convertFileSrc` from `@tauri-apps/api/core` at the top of the file, then branch in the exported function. Rollup will see the static import and keep `@tauri-apps/api/core` in the chunk graph in browser mode, which is acceptable because (a) it is already a runtime dep of `@tauri-apps/plugin-*`, (b) `convertFileSrc` is a tiny function with no side effects, and (c) avoiding the import would require an `await` which the call sites cannot provide.

```ts
import { convertFileSrc as realConvertFileSrc } from '@tauri-apps/api/core';
import { isTauri } from '../runtime';

export function convertFileSrc(path: string, protocol = 'asset'): string {
  if (isTauri()) {
    return realConvertFileSrc(path, protocol);
  }
  return path;
}
```

Use Strategy B unless the bundle audit shows it bloats production noticeably. The browser-mode passthrough returns the path unchanged — the resulting `<img src>` will fail to load, falling back to the existing placeholder image rendering in Library/Profile views. No crashes.

#### Task 1.12: Mock registry skeleton Depends on [1.1, 1.7]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/events.ts
- src/crosshook-native/src/types/settings.ts
- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/context/PreferencesContext.tsx
- docs/plans/dev-web-frontend/feature-spec.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/lib/mocks/store.ts
- src/crosshook-native/src/lib/mocks/eventBus.ts
- src/crosshook-native/src/lib/mocks/README.md

Create the mock registry skeleton with four files. None of the boot handler files exist yet (Tasks 1.13/1.14 create them) — `mocks/index.ts` may import them with empty stubs initially or use a defensive registration pattern.

`store.ts` defines the in-memory mutable state singleton:

```ts
import type { AppSettingsData, RecentFilesData } from '../../types';
import type { GameProfile } from '../../types/profile';
import { DEFAULT_APP_SETTINGS } from '../../types/settings';
import { createDefaultProfile } from '../../types/profile';

export interface MockStore {
  settings: AppSettingsData;
  recentFiles: RecentFilesData;
  profiles: Map<string, GameProfile>;
  activeProfileId: string | null;
  defaultSteamClientInstallPath: string;
}

const EMPTY_RECENT_FILES: RecentFilesData = {
  game_paths: [],
  trainer_paths: [],
  dll_paths: [],
};

let store: MockStore | null = null;

export function getStore(): MockStore {
  if (!store) {
    store = {
      settings: { ...DEFAULT_APP_SETTINGS },
      recentFiles: { ...EMPTY_RECENT_FILES },
      profiles: new Map(),
      activeProfileId: null,
      defaultSteamClientInstallPath: '/home/devuser/.steam/steam',
    };
  }
  return store;
}

export function resetStore(): void {
  store = null;
}
```

`eventBus.ts` re-exports `emitMockEvent` from `lib/events.ts` so handler files have a clean local import:

```ts
export { emitMockEvent } from '../events';
```

`index.ts` is the orchestrator. Phase 1 only needs to wire up settings + profile handlers; future tasks add more `register*` calls:

```ts
import { registerSettings } from './handlers/settings';
import { registerProfile } from './handlers/profile';

export type Handler = (args: unknown) => unknown | Promise<unknown>;

export function registerMocks(): Map<string, Handler> {
  const map = new Map<string, Handler>();
  registerSettings(map);
  registerProfile(map);
  return map;
}
```

`README.md` is the contributor entry point. Document:

- How to add a new handler (copy the existing `register<Domain>` template)
- The `getStore()` singleton and HMR reset behavior (`Vite HMR resets the store on any handler edit — clean state on save`)
- The fixture content policy (synthetic game names, Steam IDs ≥ 9999001, no real paths)
- The `?fixture=` URL switcher (Phase 3 scope, but pre-document the hook for handlers using fixture-state-aware dispatch)
- The "do not deploy" warning: `vite build --mode webdev` would ship mock code as a public website if the output were uploaded to a web host — this is an intentional foot-gun
- Cross-link to the relevant business rules (BR-1 through BR-12 in feature-spec.md)

The README does not need to be exhaustive in Phase 1 — Phase 2 contributors will iterate on it as they add handlers.

#### Task 1.13: lib/mocks/handlers/settings.ts boot handlers Depends on [1.5, 1.12]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/store.ts
- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/settings.ts
- src/crosshook-native/src/context/PreferencesContext.tsx
- docs/plans/dev-web-frontend/analysis-code.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/settings.ts

Implement the boot-critical settings mock handlers required for `PreferencesContext` to resolve at app mount. Without these, the shell never renders. The four boot-blocking commands are `settings_load`, `recent_files_load`, `default_steam_client_install_path`, `settings_save`. Plus `recent_files_save` and `settings_save_steamgriddb_key` because `PreferencesContext` calls them in user-triggered flows.

```ts
import type { Handler } from '../index';
import { getStore } from '../store';
import type { AppSettingsData, RecentFilesData } from '../../../types';

export function registerSettings(map: Map<string, Handler>): void {
  map.set('settings_load', async () => getStore().settings);

  map.set('settings_save', async (args) => {
    const next = (args as { data: AppSettingsData }).data;
    getStore().settings = { ...getStore().settings, ...next };
    return getStore().settings;
  });

  map.set('settings_save_steamgriddb_key', async (args) => {
    const { key } = args as { key: string | null };
    getStore().settings = {
      ...getStore().settings,
      has_steamgriddb_api_key: key !== null && key.trim().length > 0,
    };
    return null;
  });

  map.set('recent_files_load', async () => getStore().recentFiles);

  map.set('recent_files_save', async (args) => {
    const next = (args as { data: RecentFilesData }).data;
    getStore().recentFiles = next;
    return next;
  });

  map.set('default_steam_client_install_path', async () => getStore().defaultSteamClientInstallPath);
}
```

The mutating handlers (`settings_save`, `settings_save_steamgriddb_key`, `recent_files_save`) update the in-memory store AND return the saved payload — this matches BR-6 (mutating commands return the stored object) so optimistic-UI components that re-read after write continue to function. The `recent_files_save` argument shape (`{ data: RecentFilesData }`) matches the actual `invoke('recent_files_save', { data: nextRecentFiles })` call in `PreferencesContext.clearRecentFiles`.

#### Task 1.14: lib/mocks/handlers/profile.ts boot handlers Depends on [1.5, 1.12]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/store.ts
- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/library.ts
- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/hooks/useLibrarySummaries.ts
- src/crosshook-native/src/context/ProfileContext.tsx
- docs/plans/dev-web-frontend/analysis-code.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/profile.ts

Implement the boot-critical profile mock handlers. The minimum set required for the app shell to render through the Library route is `profile_list`, `profile_list_summaries`, `profile_list_favorites`, and `profile_load`. Phase 2's profile-mutation handlers (save/duplicate/rename/delete) are NOT in scope for this task.

Seed two synthetic profiles in the store at first call (so Library renders meaningful demo data instead of empty state by default — empty state ships as the `?fixture=empty` Phase 3 variant):

```ts
import type { Handler } from '../index';
import { getStore } from '../store';
import type { ProfileSummary } from '../../../types/library';
import type { GameProfile } from '../../../types/profile';
import { createDefaultProfile } from '../../../types/profile';

function seedDemoProfiles(): void {
  const store = getStore();
  if (store.profiles.size > 0) return;

  const alpha: GameProfile = {
    ...createDefaultProfile(),
    name: 'Test Game Alpha',
    gameName: 'Test Game Alpha',
    steamAppId: '9999001',
  };
  const beta: GameProfile = {
    ...createDefaultProfile(),
    name: 'Dev Game Beta',
    gameName: 'Dev Game Beta',
    steamAppId: '9999002',
  };
  store.profiles.set(alpha.name, alpha);
  store.profiles.set(beta.name, beta);
  store.activeProfileId = alpha.name;
}

export function registerProfile(map: Map<string, Handler>): void {
  map.set('profile_list', async () => {
    seedDemoProfiles();
    return Array.from(getStore().profiles.values());
  });

  map.set('profile_list_summaries', async (): Promise<ProfileSummary[]> => {
    seedDemoProfiles();
    return Array.from(getStore().profiles.values()).map((p) => ({
      name: p.name,
      gameName: p.gameName,
      steamAppId: p.steamAppId,
    }));
  });

  map.set('profile_list_favorites', async () => {
    seedDemoProfiles();
    return [];
  });

  map.set('profile_load', async (args) => {
    seedDemoProfiles();
    const { name } = args as { name: string };
    return getStore().profiles.get(name) ?? null;
  });
}
```

All fixture content uses obviously-synthetic names (`Test Game Alpha`, `Dev Game Beta`) and Steam IDs in the reserved synthetic range (`9999001`+) per BR-10 / W-3. Do NOT use real game names or real Steam IDs even as placeholders. The `seedDemoProfiles()` lazy-init pattern avoids re-seeding on every list call but allows HMR resets to re-create them on the next call.

#### Task 1.15: DevModeBanner component + dev-indicator.css Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/variables.css
- src/crosshook-native/src/styles/theme.css
- docs/plans/dev-web-frontend/research-ux.md
- docs/plans/dev-web-frontend/feature-spec.md

**Instructions**

Files to Create

- src/crosshook-native/src/lib/DevModeBanner.tsx
- src/crosshook-native/src/lib/dev-indicator.css

Create the two-layer dev-mode indicator. Layer 1 is a CSS-only inset outline applied to the root shell via a modifier class; Layer 2 is the fixed-position corner chip component.

`dev-indicator.css`:

```css
/* Layer 1 — inset outline on root shell. Zero layout impact (uses inset
   box-shadow, not border). Visible on all four edges. Survives screenshot
   crops because it cannot be cropped without revealing the missing border. */
.crosshook-app--webdev {
  box-shadow: inset 0 0 0 3px var(--crosshook-color-warning);
}

/* Layer 2 — fixed corner chip sizing overrides. Reuses
   .crosshook-status-chip and .crosshook-status-chip--warning base classes. */
.crosshook-dev-chip {
  position: fixed;
  bottom: 12px;
  right: 12px;
  z-index: 9999;
  min-height: 32px;
  padding: 0 10px;
  font-size: 0.78rem;
  pointer-events: none; /* non-interactive */
}
```

`DevModeBanner.tsx`:

```tsx
import './dev-indicator.css';

export interface DevModeBannerProps {
  fixture?: string; // Phase 3 will pass the active fixture name; Phase 1 always 'populated'
}

export function DevModeBanner({ fixture = 'populated' }: DevModeBannerProps) {
  return (
    <div
      className="crosshook-status-chip crosshook-status-chip--warning crosshook-dev-chip"
      role="status"
      aria-label={`Browser dev mode active. Fixture: ${fixture}`}
    >
      DEV · {fixture}
    </div>
  );
}
```

The CSS import inside the component file is the safe pattern: when `App.tsx` (Task 1.16) only renders `<DevModeBanner />` under `__WEB_DEV_MODE__`, Rollup tree-shakes the unused component and its imported CSS in production builds. The `.crosshook-app--webdev` class never appears in production stylesheets because the CSS file is only ever pulled in via this component's import.

Per UX research A-6, the chip is non-dismissable — no close button, `pointer-events: none`. Per accessibility requirements, `role="status"` (implies `aria-live="polite"`) with a descriptive `aria-label`. The chip reuses existing `.crosshook-status-chip--warning` tokens for color/contrast; verify with browser DevTools that text contrast meets WCAG AA (≥ 4.5:1) against the chip background.

#### Task 1.16: Mechanical migration of all IPC call sites Depends on [1.6, 1.7, 1.8, 1.9, 1.10, 1.11, 1.13, 1.14, 1.15]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/ipc.ts
- src/crosshook-native/src/lib/events.ts
- src/crosshook-native/src/lib/plugin-stubs/dialog.ts
- src/crosshook-native/src/lib/plugin-stubs/shell.ts
- src/crosshook-native/src/lib/plugin-stubs/fs.ts
- src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts
- src/crosshook-native/src/lib/DevModeBanner.tsx
- src/crosshook-native/src/App.tsx
- src/crosshook-native/src/main.tsx
- src/crosshook-native/src/context/PreferencesContext.tsx
- src/crosshook-native/src/utils/optimization-catalog.ts
- docs/plans/dev-web-frontend/analysis-code.md

**Instructions**

Files to Modify

- src/crosshook-native/src/App.tsx
- src/crosshook-native/src/main.tsx
- All 42 files importing `@tauri-apps/api/core` (full list in analysis-code.md "Files to Modify" section)
- All 13 files importing `@tauri-apps/api/event`
- All 6 files importing `@tauri-apps/plugin-dialog` / `-shell` / `-fs`
- src/crosshook-native/src/context/PreferencesContext.tsx (counted in the 42)
- src/crosshook-native/src/utils/optimization-catalog.ts (counted in the 42)
- src/crosshook-native/src/hooks/useLibrarySummaries.ts (Task 1.5 already removed local interface; this task migrates the invoke call)

Perform the mechanical find/replace migration. Use editor find-in-files or `sed`/`rg` — NOT codemods. The migration is purely textual: import path + function name. Group commits by transform type for reviewability even though all commits land in one PR.

**Transform 1 — invoke import (apply to all 42 files):**

```
Before: import { invoke } from '@tauri-apps/api/core';
After:  import { callCommand } from '@/lib/ipc';

Before: invoke<T>('command_name', args)
After:  callCommand<T>('command_name', args)

Before: invoke('command_name')
After:  callCommand('command_name')
```

For files that also import `convertFileSrc` from `@tauri-apps/api/core` (`hooks/useGameCoverArt.ts`, `components/profile-sections/MediaSection.tsx`), use a split import:

```
Before: import { convertFileSrc, invoke } from '@tauri-apps/api/core';
After:  import { convertFileSrc } from '@/lib/plugin-stubs/convertFileSrc';
        import { callCommand } from '@/lib/ipc';
```

**Transform 2 — listen import (apply to all 13 files):**

```
Before: import { listen } from '@tauri-apps/api/event';
After:  import { subscribeEvent } from '@/lib/events';

Before: listen<T>('event-name', handler)
After:  subscribeEvent<T>('event-name', handler)
```

Hooks that import both `invoke` (from core) and `listen` (from event) — `useLaunchState.ts`, `useProfile.ts`, `useCommunityProfiles.ts`, `useOfflineReadiness.ts`, `useUpdateGame.ts`, `useProfileHealth.ts`, `useRunExecutable.ts` — get both transforms in the same file edit.

**Transform 3 — plugin imports:**

```
Before: import { open, save } from '@tauri-apps/plugin-dialog';
After:  import { open, save } from '@/lib/plugin-stubs/dialog';

Before: import { open } from '@tauri-apps/plugin-shell';
After:  import { open } from '@/lib/plugin-stubs/shell';
```

The 6 plugin-importing files identified in analysis-code.md are: `utils/dialog.ts`, `components/CommunityBrowser.tsx`, `components/ExternalResultsSection.tsx`, `components/ProtonDbLookupCard.tsx`, `components/TrainerDiscoveryPanel.tsx`, `components/SettingsPanel.tsx`.

**Transform 4 — App.tsx surgical changes:**

- Migrate `import { listen } from '@tauri-apps/api/event'` (line 5) to `import { subscribeEvent } from '@/lib/events'`
- Migrate the `listen<OnboardingCheckPayload>('onboarding-check', ...)` call at line 67 to `subscribeEvent<OnboardingCheckPayload>(...)` (the cleanup pattern `p.then((f) => f())` works unchanged because `subscribeEvent` returns the same `Promise<UnlistenFn>` shape)
- Add `import { DevModeBanner } from '@/lib/DevModeBanner'` at the top of the file
- Wrap the root `<main>` element's `className` with a build-constant conditional, then render `<DevModeBanner />` inside `<main>` but BEFORE `<ProfileProvider>`. Concretely, the `App()` function's `return` JSX changes to:

```tsx
return (
  <main
    ref={gamepadNav.rootRef}
    className={`crosshook-app crosshook-focus-scope${__WEB_DEV_MODE__ ? ' crosshook-app--webdev' : ''}`}
  >
    {__WEB_DEV_MODE__ && <DevModeBanner />}
    <ProfileProvider>
      <ProfileHealthProvider>
        <AppShell controllerMode={gamepadNav.controllerMode} />
      </ProfileHealthProvider>
    </ProfileProvider>
  </main>
);
```

This ordering is critical: the chip must render above the provider tree so it remains visible if any provider throws during init. The `__WEB_DEV_MODE__` constant is eliminated to `false` by Rollup in production, so both the class and the chip render are dead-code-eliminated.

**Transform 5 — main.tsx eager import:**

- Add `import '@/lib/plugin-stubs/convertFileSrc';` as a side-effect import near the top, before any React rendering. This ensures the synchronous `convertFileSrc` shim is in the module graph before any `useGameCoverArt` or `MediaSection` render happens.

**Transform 6 — PreferencesContext parallel boot:**

- The four `invoke()` calls in `loadPreferences()` (lines 43-46), `persistSettings` (lines 106-107), `handleSteamGridDbApiKeyChange` (line 135), and `clearRecentFiles` (line 157) all migrate via Transform 1. The `Promise.all` shape, `active` flag pattern, and `formatError` helper stay unchanged.

**Verification (run in order after migration):**

```bash
# 1. No stale imports left behind
rg "from ['\"]@tauri-apps/api/core['\"]" src/crosshook-native/src/
# Must return ONLY: src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts
# (the only file that legitimately keeps the import — see Strategy B in Task 1.11)

rg "from ['\"]@tauri-apps/api/event['\"]" src/crosshook-native/src/
# Must return: 0 hits (only lib/events.ts uses it via dynamic import)

rg "from ['\"]@tauri-apps/plugin-(dialog|shell|fs)['\"]" src/crosshook-native/src/
# Must return ONLY: src/crosshook-native/src/lib/plugin-stubs/{dialog,shell,fs}.ts

# 2. TypeScript still resolves every import + every type (catches bad @/ paths,
#    missing tsconfig.json paths entry, and any type drift from the migration).
#    Run this BEFORE the smoke tests — a bad alias surfaces as a runtime error
#    on one specific page, which is much harder to triage than a tsc error.
cd src/crosshook-native && npx tsc --noEmit

# 3. Rust backend still green (no Rust changes expected, but the 60+ file TS
#    diff warrants the check).
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

Then two manual smoke tests:

1. `./scripts/dev-native.sh --browser` — click through all 9 routes; verify no console errors and the dev indicator chip is visible on every route.
2. `./scripts/dev-native.sh` (Tauri mode) — verify the same 9 routes work, with the chip ABSENT, and the real `onboarding-check` event still fires on first run.

#### Task 1.17: CI verify:no-mocks grep step Depends on [1.16]

**READ THESE BEFORE TASK**

- .github/workflows/release.yml
- docs/plans/dev-web-frontend/research-security.md
- docs/plans/dev-web-frontend/feature-spec.md

**Instructions**

Files to Modify

- .github/workflows/release.yml

Add a `verify:no-mocks` grep sentinel step after the "Build native AppImage" step in the existing release workflow. This is the **authoritative production safety control** per security finding W-1 — even though the `__WEB_DEV_MODE__` define + dynamic-import dead-branch pattern should keep mock code out of `dist/`, Rollup's dynamic-import handling has known edge cases (Vite #11080), and the grep is the deterministic fail-safe.

Insert this step in `release.yml` after the Build native AppImage step and before any artifact upload step:

```yaml
- name: Verify no mock code in production bundle
  run: |
    echo "Checking dist/assets/ for mock-mode sentinel strings..."
    if grep -rl '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' \
        src/crosshook-native/dist/assets/*.js 2>/dev/null; then
      echo "::error::Mock code found in production bundle — refusing to ship" >&2
      echo "This is a CRITICAL security failure. The __WEB_DEV_MODE__ define" >&2
      echo "or the dynamic-import dead-branch failed to eliminate mock code." >&2
      echo "Sentinel strings checked: [dev-mock], getMockRegistry, registerMocks, MOCK MODE" >&2
      exit 1
    fi
    echo "Bundle clean — no mock strings found"
```

The four sentinel strings (`[dev-mock]`, `getMockRegistry`, `registerMocks`, `MOCK MODE`) are chosen to be unique to the mock layer and unlikely to appear in production code. The `2>/dev/null` suppresses grep's "no files found" error when `dist/assets/` is empty (which would be a different failure mode caught earlier).

Verify the step locally before relying on CI: run `./scripts/build-native.sh --binary-only`, then run the same grep command from your terminal — it must produce zero hits.

#### Task 1.18: AGENTS.md Commands block update Depends on [1.16]

**READ THESE BEFORE TASK**

- AGENTS.md
- CLAUDE.md
- docs/plans/dev-web-frontend/feature-spec.md

**Instructions**

Files to Modify

- AGENTS.md

Update the AGENTS.md "Commands (short reference)" block to add `./scripts/dev-native.sh --browser` as a new entry. The current block is at AGENTS.md and lists 6 commands; insert the new entry as the second command (after `./scripts/dev-native.sh` and before `./scripts/build-native.sh`):

```
./scripts/dev-native.sh
./scripts/dev-native.sh --browser    # browser-only dev mode (no Rust toolchain), loopback only
./scripts/build-native.sh
./scripts/build-native-container.sh
./scripts/build-native.sh --binary-only
./scripts/install-native-build-deps.sh
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core
```

Add a 1-paragraph "Browser Dev Mode" section directly under the Commands block (or in a logical nearby section) that explains:

- The flag starts Vite at `http://localhost:5173` with all IPC calls served by hand-rolled mock handlers
- Loopback only — `--host 0.0.0.0` is unsupported per security policy
- Real Tauri behavior must be re-verified with `./scripts/dev-native.sh` (no flag) before merging UI changes
- Pointer to `src/crosshook-native/src/lib/mocks/README.md` for adding new handlers
- Mention the CI sentinel: production AppImage builds run `verify:no-mocks` to refuse any bundle containing mock code

Do NOT change CLAUDE.md in this task — the existing rules already cover this feature (persistence boundary, scroll-container registry, commit prefix policy). The PR description is where the runtime-only persistence boundary statement goes per CLAUDE.md.

### Phase 2: Handler Fan-out (independent sub-PRs)

Phase 2 expands mock handler coverage across all ~13 domain groups so every major route renders meaningful data. Each task is its own sub-PR; all 13 are fully independent of each other and only depend on Phase 1 being merged. Order of execution within Phase 2 is by iteration value (Launch first because it unlocks the launch flow, then Profiles for mutation flows, etc.) but multiple sub-PRs can be in flight simultaneously. Each handler file follows the same pattern: imports `getStore()`, exports `register<Domain>(map)`, gets wired into `lib/mocks/index.ts`, and emits events via `emitMockEvent` from `eventBus.ts` for any operation that triggers state transitions in the real backend.

#### Task 2.1: Launch handlers + launch event bus Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/lib/mocks/store.ts
- src/crosshook-native/src/lib/mocks/eventBus.ts
- src/crosshook-native/src/types/launch.ts
- src/crosshook-native/src/hooks/useLaunchState.ts
- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/components/pages/LaunchPage.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/launch.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement mock handlers for the launch domain: `launch_game`, `launch_trainer`, `preview_launch`, `validate_launch`, `check_game_running`, `verify_trainer_hash`, `check_gamescope_session`. Each command returns synthetic `LaunchResult` data using types from `types/launch.ts`. After `launch_game` is called, schedule a sequence of `emitMockEvent` calls via `setTimeout` to fire `launch-log` (multiple lines), `launch-diagnostic`, and `launch-complete` events with realistic delays (~100-500ms apart) so the existing console drawer and launch state hooks render meaningful state transitions. Wire `registerLaunch(map)` into `lib/mocks/index.ts`. Verify by running `./scripts/dev-native.sh --browser`, navigating to the Launch page, and clicking the launch button — the console drawer should populate with mock log lines.

#### Task 2.2: Profile mutation handlers + profiles-changed event Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/handlers/profile.ts
- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/hooks/useProfile.ts
- src/crosshook-native/src/components/pages/ProfilesPage.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/lib/mocks/handlers/profile.ts

Extend the existing `handlers/profile.ts` (created in Task 1.14) with mutation handlers: `profile_save`, `profile_duplicate`, `profile_rename`, `profile_delete`, `profile_set_favorite`, plus config history/diff/rollback commands and optimization preset commands as referenced in research-business.md. Each mutation updates the in-memory store via `getStore().profiles.set(...)`, then calls `emitMockEvent('profiles-changed', { ... })` so subscribed hooks re-fetch the list. Mutations return the saved profile per BR-6.

#### Task 2.3: Health dashboard handlers + events Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/health.ts
- src/crosshook-native/src/hooks/useProfileHealth.ts
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/health.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`, `check_version_status`, `acknowledge_version_change`. Use `EnrichedHealthSummary` and `CachedHealthSnapshot[]` types from `types/health.ts`. Emit `profile-health-batch-complete` and `version-scan-complete` events from the appropriate handlers. Wire into `lib/mocks/index.ts`.

#### Task 2.4: Onboarding handlers + onboarding-check event Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/onboarding.ts
- src/crosshook-native/src/hooks/useOnboarding.ts
- src/crosshook-native/src/components/OnboardingWizard.tsx
- src/crosshook-native/src/App.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/onboarding.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `check_readiness`, `dismiss_onboarding`, `check_version_status` (if not covered by Task 2.3). On module init, optionally synthesize the `onboarding-check` event after a short delay if `?onboarding=show` is in the URL — this is the trigger for the Phase 3 onboarding debug toggle, but the handler can pre-build the hook so Phase 3 only needs to flip the flag. Use `ReadinessCheckResult` type from `types/onboarding.ts`.

#### Task 2.5: Install flow handlers + events Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/install.ts
- src/crosshook-native/src/hooks/useInstallGame.ts
- src/crosshook-native/src/components/pages/InstallPage.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/install.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `install_game`, `validate_install_request`, `install_default_prefix_path`, plus the `install-*` event sequence (started, progress, complete, error). Use `InstallStatus` from `types/install.ts`. Stagger events via setTimeout to drive realistic progress UI.

#### Task 2.6: Update flow handlers + events Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/hooks/useUpdateGame.ts

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/update.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `update_game`, `validate_update_request`, `cancel_update`, plus `update-*` event sequence. Pattern matches install flow.

#### Task 2.7: Proton stack handlers Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/proton.ts
- src/crosshook-native/src/hooks/useProtonInstalls.ts
- src/crosshook-native/src/hooks/useProtonMigration.ts

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/proton.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `list_proton_installs`, `check_proton_migrations`, `apply_proton_migration`, `apply_batch_migration`. Use `ProtonInstallOption[]` from `types/proton.ts`. Return 2-3 synthetic Proton installations with versions like `GE-Proton9-X` and paths like `/mock/compatibilitytools.d/GE-Proton9-1`.

#### Task 2.8: ProtonUp handlers Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/hooks/useProtonUp.ts

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/protonup.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `protonup_list_available_versions`, `protonup_install_version`, `protonup_get_suggestion`. Synthetic version data with placeholder release notes.

#### Task 2.9: ProtonDB lookup handlers Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/protondb.ts
- src/crosshook-native/src/hooks/useProtonDbLookup.ts
- src/crosshook-native/src/hooks/useProtonDbSuggestions.ts

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/protondb.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `protondb_lookup`, `protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion`. Return synthetic Gold/Platinum/Silver tier data for the seeded demo profiles. For `accept` / `dismiss`, track the profile name in `store.dismissedSuggestions: Set<string>` and return `null`; no event emission is required for Phase 2.

**Coordination with Task 2.3**: ProtonDB commands must be registered **only in `handlers/protondb.ts`**, NOT in `handlers/health.ts`. If Task 2.3 is implemented first and places ProtonDB commands in `health.ts`, this task MUST move them to `protondb.ts` and delete the `health.ts` entries — otherwise `registerMocks()` will call `map.set('protondb_lookup', ...)` twice, silently overwriting the first handler. Resolve this by grep-checking `handlers/health.ts` before writing `handlers/protondb.ts` and making this a cross-task coordination comment in the PR description.

#### Task 2.10: Community handlers Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/hooks/useCommunityProfiles.ts
- src/crosshook-native/src/components/pages/CommunityPage.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/community.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `community_list_profiles`, `community_list_indexed_profiles`, `community_sync`, `community_add_tap`, `community_prepare_import`, `community_export_profile`. Return 2-3 synthetic community profiles with realistic-looking names but obviously-fake content.

#### Task 2.11: Launcher export handlers Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/launcher.ts
- src/crosshook-native/src/hooks/useLauncherManagement.ts
- src/crosshook-native/src/components/LauncherExport.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/launcher.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement `list_launchers`, `check_launcher_exists`, `validate_launcher_export`, `export_launchers`, `preview_launcher_*`, `delete_launcher*`. Synthetic launcher entries; export operations log a warn but return success.

#### Task 2.12: Library media handlers (cover art, metadata, steam) Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/game-metadata.ts
- src/crosshook-native/src/hooks/useGameCoverArt.ts
- src/crosshook-native/src/hooks/useGameMetadata.ts

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/library.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement art-fetch, metadata-fetch, and Steam-related commands. The `convertFileSrc` passthrough means cover-art `<img src>` will fall back to placeholders — that is acceptable.

#### Task 2.13: Remaining domain handlers (discovery, run-executable, prefix, diagnostics, catalog) Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/types/discovery.ts
- src/crosshook-native/src/types/prefix-storage.ts
- src/crosshook-native/src/types/run-executable.ts
- src/crosshook-native/src/utils/optimization-catalog.ts
- src/crosshook-native/src/hooks/useTrainerDiscovery.ts
- src/crosshook-native/src/hooks/useTrainerTypeCatalog.ts

**Instructions**

Files to Create

- src/crosshook-native/src/lib/mocks/handlers/system.ts

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts

Implement the remaining smaller domain commands: trainer discovery, run-executable, prefix storage, diagnostics, optimization catalog (`get_optimization_catalog` returning the existing `OptimizationCatalogPayload`). Group into a single `system.ts` handler file because each is small. Wire into `lib/mocks/index.ts`.

### Phase 3: Polish (independent sub-PRs)

Phase 3 adds fixture variants, debug toggles, coverage tooling, docs, and optional test automation. All 7 tasks are independent and can ship as separate small PRs. The fixture switcher (3.1) and debug toggles (3.2) build on the Phase 2 handler foundation; the rest are tooling/docs work.

#### Task 3.1: Fixture state switcher (?fixture=populated|empty|error|loading) Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/lib/DevModeBanner.tsx
- docs/plans/dev-web-frontend/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/lib/mocks/handlers/profile.ts
- src/crosshook-native/src/lib/mocks/handlers/launch.ts
- src/crosshook-native/src/lib/DevModeBanner.tsx
- src/crosshook-native/src/App.tsx

Add a module-scope `getActiveFixture()` function in `lib/mocks/index.ts` that reads `URLSearchParams(window.location.search).get('fixture')` once at module init and returns one of `'populated' | 'empty' | 'error' | 'loading'` (defaulting to `'populated'`). Pass the active fixture to `<DevModeBanner fixture={...} />` from App.tsx so the chip label updates. Each handler that supports fixture variants checks the active fixture and dispatches:

- `populated`: returns demo data (current behavior)
- `empty`: returns empty arrays / null
- `error`: throws synthetic errors for fallible commands; reads still succeed so the shell renders
- `loading`: returns a never-resolving promise (`new Promise(() => {})`) so loading states stay visible

Shell-critical reads (`settings_load`, `profile_list`) must continue to resolve in `error` state per BR-11 so the app shell always renders.

#### Task 3.2: Orthogonal debug toggles (?errors, ?delay, ?onboarding) Depends on [3.1, 2.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/lib/mocks/handlers/onboarding.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/src/lib/DevModeBanner.tsx

Add a `wrapHandler()` middleware in `lib/mocks/index.ts` applied to every registered handler. The wrapper:

- Reads `?delay=<ms>` and wraps handler results in `await new Promise(r => setTimeout(r, delay))` if set
- Reads `?errors=true` and rejects mutating commands (filter via a `READ_COMMANDS` set listing all read-only command names) if set; reads always succeed
- Reads `?onboarding=show` and synthesizes an `onboarding-check` event on first call via `emitMockEvent`

Update `<DevModeBanner />` chip label to reflect active toggles, e.g., `DEV · empty · errors · 800ms`. Document the toggles in `lib/mocks/README.md`.

#### Task 3.3: Handler-coverage check script (dev:browser:check) Depends on [1.16]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/lib/mocks/index.ts
- src/crosshook-native/package.json
- src/crosshook-native/src-tauri/src/lib.rs (or wherever #[tauri::command] handlers are registered)

**Instructions**

Files to Create

- scripts/check-mock-coverage.sh

Files to Modify

- src/crosshook-native/package.json

Create a script that grep-extracts all `#[tauri::command]` function names from the Rust sources (`crosshook-core/src/**/*.rs` or `src-tauri/src/**/*.rs`) and diffs them against the keys registered in `lib/mocks/index.ts` via `registerMocks()`. Output two lists: commands missing a mock handler, and mock handlers without a corresponding Rust command (drift in the other direction). Add `"dev:browser:check": "bash ../../scripts/check-mock-coverage.sh"` to `package.json`. This is a contributor convenience tool — not a CI gate yet.

#### Task 3.4: Project README Browser Dev Mode section Depends on [1.16]

**READ THESE BEFORE TASK**

- README.md
- src/crosshook-native/src/lib/mocks/README.md

**Instructions**

Files to Modify

- README.md

Add a "Browser Dev Mode" section to the project README that describes the feature, the `--browser` flag, the `http://localhost:5173` URL, the no-Rust-required nature, and a link to `src/crosshook-native/src/lib/mocks/README.md` for contributors. Keep it short — 2-3 paragraphs.

#### Task 3.5: Optional Playwright smoke test Depends on [3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/package.json
- docs/plans/dev-web-frontend/research-recommendations.md

**Instructions**

Files to Create

- src/crosshook-native/playwright.config.ts
- src/crosshook-native/tests/smoke.spec.ts

Files to Modify

- src/crosshook-native/package.json

Add Playwright as a `devDependency` (this is the first frontend test framework — exception to the "no new dependencies" Phase 1 rule because Phase 3 explicitly opens scope for it). Configure to start the dev server on `vite --mode webdev`, navigate through all 9 routes, and screenshot each. Add a npm script `"test:smoke": "playwright test"`. This unlocks future visual-regression workflows but is optional for Phase 3 — only ship if the team wants it.

#### Task 3.6: Refactor follow-up issue for direct invoke components Depends on [1.16]

**READ THESE BEFORE TASK**

- docs/plans/dev-web-frontend/research-practices.md
- src/crosshook-native/src/components/pages/LaunchPage.tsx
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx
- src/crosshook-native/src/components/ProfileActions.tsx
- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx

**Instructions**

Open a `refactor:` GitHub issue tracking the 5 components (per analysis-code.md, not 13 as feature-spec.md states) that call `callCommand()` directly instead of delegating to a hook. The exact files are:

- src/crosshook-native/src/components/pages/LaunchPage.tsx
- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx
- src/crosshook-native/src/components/ProfileActions.tsx
- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx

Per CLAUDE.md, use the YAML form template at `.github/ISSUE_TEMPLATE/feature_request.yml` (or a refactor-focused variant if one exists). Tag with `type:refactor`, `area:ui`, `priority:low`.

**Known `gh` CLI limitation** (documented in CLAUDE.md and AGENTS.md): `gh issue create --template feature_request.yml` may return `no templates found` even though the YAML form template is valid. If this happens, DO NOT fall back to a vague `--title`-only issue — that is an explicit CLAUDE.md violation. Instead, use the GitHub API directly (`gh api repos/:owner/:repo/issues -F title=... -F body=...` with a HEREDOC body that mirrors the form's fields: Problem, Proposed Solution, Acceptance Criteria, Alternatives Considered), then apply labels via `gh issue edit NNN --add-label type:refactor,area:ui,priority:low`.

Issue body must include: the 5 component file list, the architectural rationale (business logic in components should be extracted into hooks for reuse and testability), a note that this is a follow-up to the dev-web-frontend Phase 1 PR, and a link back to `research-practices.md` in the plan directory. No code changes in this task — issue creation only.

#### Task 3.7: Fixture-content CI lint Depends on [1.16]

**READ THESE BEFORE TASK**

- .github/workflows/release.yml
- src/crosshook-native/src/lib/mocks/handlers
- docs/plans/dev-web-frontend/research-security.md

**Instructions**

Files to Modify

- .github/workflows/release.yml (or create a new workflow file)

Add a CI step that greps `src/crosshook-native/src/lib/mocks/` for SteamID64 patterns (`\b[0-9]{17}\b`) and home-path patterns (`/home/`, `/Users/`, `/users/`) — fail the build on any hit. This catches accidental real-data leaks in fixtures before they ship. Scope strictly to `lib/mocks/` so it doesn't false-positive on legitimate SteamID handling elsewhere.

## Advice

- **The promise latch in `ensureMocks()` is non-obvious but critical.** `PreferencesContext` fires 3 parallel `callCommand` calls on mount. Without `let mocksPromise: Promise<...> | null = null` serializing the first `import('./mocks')`, all three calls would race to initialize the registry, and `Map` operations would interleave unpredictably. Test this specifically by adding a `console.log` inside `registerMocks()` and verifying it only runs once across the parallel boot.
- **Task 1.5 (ProfileSummary export) has zero upstream dependencies but blocks Tasks 1.13 and 1.14.** It is the most commonly missed dependency in feature dependency graphs because it modifies existing files rather than creating new ones. Schedule it explicitly in wave 1 even though it doesn't fit the "create new lib/ files" mental model of the foundation phase.
- **`convertFileSrc` MUST stay as a static import** — Strategy B in Task 1.11 keeps `@tauri-apps/api/core` in the browser-mode bundle, which is acceptable because the function is called synchronously from React render paths and cannot be async-wrapped. This is the one exception to the "no Tauri imports in browser-mode bundle" rule.
- **`App.tsx` rendering order matters.** `<DevModeBanner />` must render OUTSIDE `<ProfileProvider>` so it appears even if context init fails. If you put it inside, it disappears whenever a provider throws — which is exactly when the chip is most useful.
- **Vite's `defineConfig` must change shape from object to function** in Task 1.1 — `defineConfig({ ... })` becomes `defineConfig(({ mode }) => ({ ... }))`. This is a structural change to a critical config file; review carefully.
- **`types/library.ts` is missing from the `types/index.ts` barrel.** This is a pre-existing bug that this plan fixes incidentally in Task 1.5. Don't be surprised if other code starts breaking when the barrel includes new types — there should be no conflicts because the file currently has only `LibraryViewMode` and `LibraryCardData`, but verify after adding.
- **WebKitGTK ≠ Chrome.** Browser dev mode is a design tool, not a parity guarantee. Always re-verify in `./scripts/dev-native.sh` (no flag) before merging any UI change. The handful of differences (scroll physics, `color-mix()` support, font rendering, focus styles) will bite eventually.
- **The CI grep sentinel is the authoritative production safety control, not the `__WEB_DEV_MODE__` define.** The define is the primary mechanism (Rollup dead-code elimination), but Rollup's dynamic-import handling has known edge cases (Vite #11080). Treat the grep as the deterministic fail-safe and never remove it from `release.yml` even if the define-guard mechanism becomes more robust.
- **Per D2, Phase 1 ships in ONE PR.** Do not split the adapter from the migration into separate PRs — without all of it, the app is a blank screen at `http://localhost:5173`. Group commits by task ID for reviewability, but the merge unit is the whole PR.
- **All commits under `docs/plans/dev-web-frontend/` MUST use the `docs(internal):` prefix per CLAUDE.md.** This includes commits made during the implementation phase that update plan tracking. Implementation commits use `feat(ui):`, `feat(build):`, or `refactor(...):` depending on the area. The `verify:no-mocks` CI step lands as a `feat(build):` or `ci:` commit.
- **The PR description must explicitly state the persistence boundary** per CLAUDE.md: "This feature adds no persisted data. It is additive to the dev workflow only, and the production Tauri build is byte-identical whether or not the feature is present, enforced by CI sentinel grep on the AppImage bundle." Skipping this is a CLAUDE.md violation that should fail review.
- **Phase 2 handler PRs are fully parallel** but coordinate on `lib/mocks/index.ts` — every Phase 2 PR adds a `register<Domain>(map)` call to the same file. Expect merge conflicts in `index.ts` and resolve by appending each domain registration.
- **Per BR-10 / W-3, fixture content policy is strict.** Synthetic game names (`Test Game Alpha`), Steam IDs ≥ `9999001` (outside the valid range), no real paths. PR review enforces in Phase 1 and 2; Task 3.7 adds CI grep enforcement in Phase 3.
- **No frontend test framework exists yet.** The verification command for Phase 1 is `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` (Rust backend tests, must still pass after the migration) plus manual smoke tests. Playwright is Phase 3 scope (Task 3.5) and optional even there.
