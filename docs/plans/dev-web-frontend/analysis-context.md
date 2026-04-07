# Context Analysis: dev-web-frontend

## Executive Summary

This feature introduces a browser-only Vite dev mode for CrossHook's React frontend by inserting a thin IPC adapter layer (`lib/runtime.ts`, `lib/ipc.ts`, `lib/events.ts`, `lib/plugin-stubs/`, `lib/mocks/`) that branches on `isTauri()` at runtime and on `__WEB_DEV_MODE__` at build time. The full migration covers 84 `invoke(` call sites (42 files), 16 `listen(` call sites (13 files), and 3 plugin package imports (6 files) — all mechanical find/replace — plus a CI grep sentinel as the authoritative production-safety gate. Work is structured in 3 phases: Phase 1 (single PR: adapter + migration + boot handlers + CI gate), Phase 2 (~13 domain handler sub-PRs), Phase 3 (fixture variants, debug toggles, coverage tooling).

---

## Architecture Context

- **System Structure**: New `src/crosshook-native/src/lib/` directory containing `runtime.ts` (6 lines, zero deps), `ipc.ts` (~25 lines), `events.ts` (~35 lines), `plugin-stubs/{dialog,shell,fs,convertFileSrc}.ts`, `mocks/index.ts` (orchestrator + wrapHandler), `mocks/store.ts`, `mocks/eventBus.ts`, `mocks/README.md`, and `mocks/handlers/<domain>.ts` per domain. No `lib/` directory exists today — it is created entirely in Phase 1.
- **Data Flow**: Call site imports `callCommand` from `@/lib/ipc` → `callCommand` calls `isTauri()` from `lib/runtime.ts` → if true, dynamic-imports `@tauri-apps/api/core.invoke` and forwards; if false, calls `ensureMocks()` which dynamic-imports `./mocks` (behind `!__WEB_DEV_MODE__` guard) → lazily builds `Map<string, Handler>` via `registerMocks()` → dispatches to domain handler; equivalent flow for `subscribeEvent` via `lib/events.ts`. `emitMockEvent(name, payload)` is called from handler files to fan out to the in-process `browserBus`.
- **Integration Points**: `vite.config.ts` gains mode-conditional `define: { __WEB_DEV_MODE__: mode === 'webdev' }` + `resolve.alias: { '@': './src' }` + webdev `server.host = '127.0.0.1'` + `strictPort = true`. `package.json` gains `"dev:browser": "vite --mode webdev"`. `scripts/dev-native.sh` gains `--browser` / `--web` case branch executing `npm run dev:browser` with no `cargo`/`tauri` calls. `release.yml` gains `verify:no-mocks` step. `App.tsx` gains `__WEB_DEV_MODE__`-gated `.crosshook-app--webdev` className and `<DevModeChip />` render above `<ProfileProvider>`. `tsconfig.json` gains `"paths": { "@/*": ["./src/*"] }`.

---

## Resolved Decisions (D1-D6)

Treat these as settled. Do not re-litigate.

- **D1** — Flag name: `--browser` primary, `--web` alias; `--dev` rejected as too generic. `AGENTS.md` and help text use `--browser` as canonical.
- **D2** — Phase 1 is a single PR: adapter + plugin stubs + mock registry skeleton + boot-critical handlers + mechanical migration + CI gate shipped together. Do not split adapter from migration.
- **D3** — Build-time guard: `__WEB_DEV_MODE__` Vite `define` constant (`mode === 'webdev'`). `ensureMocks()` opens with `if (!__WEB_DEV_MODE__) throw new Error('[dev-mock] mock layer invoked in non-webdev build — check dev:browser script passes --mode webdev')` as misconfiguration safety net.
- **D4** — Plugin stub semantics: `dialog.open`/`save` returns `null` + `console.warn('[dev-mock] dialog suppressed')` (mimics cancel); destructive ops (`shell.execute`, `fs.writeFile`, `fs.removeFile`, `fs.rename`) `throw` — silent no-ops are banned per BR-8.
- **D5** — `@/` Vite path alias added in Phase 1; matching `tsconfig.json` paths entry `"@/*": ["./src/*"]` required or TypeScript rejects the new imports. Migration uses `@/lib/ipc`, `@/lib/events`, `@/lib/plugin-stubs/*` specifiers throughout.
- **D6** — CSP: accept Chrome/Firefox defaults for Phase 1. Re-verify in `./scripts/dev-native.sh` (WebKitGTK) before merging any UI change. Document in `lib/mocks/README.md`.

---

## Critical Files Reference

**Phase 1 — new files:**

- `src/crosshook-native/src/lib/runtime.ts`: `isTauri()` probe; single source of truth; zero deps; prevents circular imports
- `src/crosshook-native/src/lib/ipc.ts`: `callCommand<T>` adapter with `ensureMocks()` promise latch (prevents concurrent mock init from parallel boot calls)
- `src/crosshook-native/src/lib/events.ts`: `subscribeEvent<T>` + `emitMockEvent` + module-scope `browserBus`
- `src/crosshook-native/src/lib/plugin-stubs/dialog.ts`: `null` + warn in browser
- `src/crosshook-native/src/lib/plugin-stubs/shell.ts`: `throw` on destructive ops in browser
- `src/crosshook-native/src/lib/plugin-stubs/fs.ts`: read ops return stubs; write/delete ops `throw`
- `src/crosshook-native/src/lib/plugin-stubs/convertFileSrc.ts`: synchronous passthrough `(path) => path` in browser (NOT covered by Vite alias — must be explicit import rewrite)
- `src/crosshook-native/src/lib/mocks/index.ts`: `registerMocks()` + `wrapHandler()` + fixture-state resolver (module-scope `URLSearchParams` init + `READ_COMMANDS` set)
- `src/crosshook-native/src/lib/mocks/store.ts`: `MockStore` type + `getStore()` singleton
- `src/crosshook-native/src/lib/mocks/handlers/settings.ts`: boot-critical; uses `DEFAULT_APP_SETTINGS`
- `src/crosshook-native/src/lib/mocks/handlers/profile.ts`: boot-critical; uses `createDefaultProfile()`
- `src/crosshook-native/src/lib/DevModeBanner.tsx` (or `DevModeChip.tsx`): fixed-position corner chip + layer 1 `dev-indicator.css`

**Phase 1 — modified files:**

- `src/crosshook-native/vite.config.ts`: `define`, `resolve.alias`, `server.host`/`strictPort` — touched by multiple Phase 1 sub-tasks; coordinate sequencing
- `src/crosshook-native/package.json`: `"dev:browser"` script — coordination hotspot
- `src/crosshook-native/tsconfig.json`: `paths` entry — coordination hotspot
- `src/crosshook-native/src/App.tsx`: `__WEB_DEV_MODE__` className + `<DevModeChip />` + `subscribeEvent` migration for `onboarding-check` at line 67 — coordination hotspot
- `src/crosshook-native/src/vite-env.d.ts`: `declare const __WEB_DEV_MODE__: boolean`
- `src/crosshook-native/src/context/PreferencesContext.tsx`: highest-priority migration target; 3-command parallel boot at lines 43-46 must resolve from mock handlers before shell renders
- `src/crosshook-native/src/types/library.ts`: must export `ProfileSummary` (currently local to `hooks/useLibrarySummaries.ts`) — unblocks typed mock handlers
- `src/crosshook-native/src/utils/optimization-catalog.ts`: non-hook `invoke` site; must migrate same as hooks
- `scripts/dev-native.sh`: `--browser`/`--web` case branch
- `.github/workflows/release.yml`: `verify:no-mocks` grep step
- `AGENTS.md`: Commands reference block update

---

## Patterns to Follow

- **IPC Adapter (Strategy + Runtime Branch)**: `callCommand<T>(name, args)` probes `isTauri()`, dispatches to real invoke or mock map. Promise latch in `ensureMocks()` serializes concurrent first calls (critical for `PreferencesContext` 3-parallel-command boot).
- **Build-Time Dead-Code Branch**: `declare const __WEB_DEV_MODE__: boolean` + `if (!__WEB_DEV_MODE__) throw` inside `ensureMocks()` + dynamic `import(/* @vite-ignore */ './mocks')` — Rollup sees `if (false)` and drops the subtree before chunk graph construction.
- **Mock Registry Fan-In**: `register*(map: Map<string, Handler>)` functions per domain populate the shared map; `registerMocks()` in `index.ts` orchestrates all calls; `wrapHandler()` applies `delay`, `errors`, and debug logging cross-cutting to every handler.
- **In-Memory MockStore Singleton**: module-scope mutable object in `store.ts`, `getStore()` accessor; HMR on any `lib/mocks/` file resets it (intentional — clean state on fixture edits).
- **Plugin Stub Re-Export**: Tauri branch does `await import('@tauri-apps/plugin-*')` and re-exports; browser branch returns `null`/warns or `throw`s. `convertFileSrc` is synchronous — no dynamic import possible; static passthrough only.
- **Two-Layer Dev Indicator**: `.crosshook-app--webdev { box-shadow: inset 0 0 0 3px var(--crosshook-color-warning) }` (zero layout impact) + `<DevModeChip />` at `position: fixed; bottom: 12px; right: 12px; z-index: 9999` with `role="status"`. Must render in `App.tsx` above `<ProfileProvider>` so it survives provider failures and appears before async fixture loading.
- **CI Grep Sentinel**: `grep -rl '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' src/crosshook-native/dist/assets/*.js` in `release.yml` after AppImage build — authoritative production safety control.

---

## Cross-Cutting Concerns

- **Security (CRITICAL — production leak)**: Three-layer defense: (1) `__WEB_DEV_MODE__` define = `false` in production builds, (2) dynamic `import('./mocks')` inside `!__WEB_DEV_MODE__` dead branch, (3) CI grep sentinel. All three must ship in Phase 1. The CI grep is the authoritative control — `__WEB_DEV_MODE__` is the primary mechanism.
- **Security (W-2 — LAN exposure)**: `vite.config.ts` must hard-code `server.host = '127.0.0.1'` and `strictPort = true` for webdev mode. `--host 0.0.0.0` is explicitly unsupported; document in `scripts/dev-native.sh` help text and `AGENTS.md`.
- **Security (W-3 — fixture content)**: Game titles must use placeholder names (`Test Game Alpha`), Steam App IDs must be ≥ `9999001` (outside valid range), no real paths. Enforced by PR review checklist on `lib/mocks/` changes. Phase 3 adds CI grep scoped to `lib/mocks/` for SteamID64 pattern and home paths.
- **Persistence**: Feature is runtime-only — no TOML settings, no SQLite tables, no migrations, no disk writes. PR description must explicitly state this per CLAUDE.md persistence boundary requirements.
- **Testing**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` must still pass after Phase 1 migration (no Rust changes expected, but the large TS diff warrants verification). No frontend test framework is currently configured.
- **Scroll registry**: `<DevModeChip />` uses `position: fixed` — NOT an `overflow-y: auto` container — so no `useScrollEnhance.ts` `SCROLLABLE` selector registration is required. Any future dev-mode panel that does introduce a scroll region must be registered.
- **`dev:browser` footgun**: plain `vite` (without `--mode webdev`) sets `__WEB_DEV_MODE__ = false`, which makes `ensureMocks()` throw a confusing internal error. The npm script hard-codes `vite --mode webdev` to prevent this. Document in `lib/mocks/README.md`.

---

## Parallelization Opportunities

- **Within Phase 1 (can parallelize)**:
  - `lib/runtime.ts` + `lib/ipc.ts` + `lib/events.ts` (core adapter, zero external deps)
  - `lib/plugin-stubs/dialog.ts` + `shell.ts` + `fs.ts` + `convertFileSrc.ts` (independent of each other and of core adapter)
  - `lib/mocks/store.ts` + `lib/mocks/eventBus.ts` + `lib/mocks/README.md` (mock infrastructure)
  - `lib/mocks/handlers/settings.ts` + `lib/mocks/handlers/profile.ts` (can start once `store.ts` exists)
  - `DevModeChip.tsx` + `dev-indicator.css` (only needs `lib/runtime.ts` to exist)
- **Phase 1 — must sequence**:
  - Mechanical call-site migration (task 7 in spec) must wait for `lib/ipc.ts`, `lib/events.ts`, and plugin stubs to land — these are the import targets
  - `App.tsx` wiring (`__WEB_DEV_MODE__` className + chip render) must wait for `DevModeChip.tsx` to exist
  - CI gate step (`release.yml`) can be authored in parallel but only tested after Phase 1 adapter is in place
- **Coordination hotspots** (multiple Phase 1 sub-tasks touch these — recommend one person owns each or explicit sequencing):
  - `vite.config.ts`: `define` + `resolve.alias` + `server` config changes
  - `App.tsx`: `subscribeEvent` migration + `__WEB_DEV_MODE__` className + `<DevModeChip />` render
  - `package.json`: `dev:browser` script
  - `tsconfig.json`: `paths` entry
- **Within Phase 2**: ~13 domain handler files are independent of each other; multiple sub-PRs can be in flight simultaneously. Each adds one `handlers/<domain>.ts` file and wires it into `mocks/index.ts`.
- **Within Phase 3**: All tasks are independent; can ship as separate small PRs.

---

## Implementation Constraints

- **No new npm dependencies**: every library evaluated was rejected; adapter uses only `@tauri-apps/api` (already present at `^2.0.0`).
- **Single PR for Phase 1**: per D2 — adapter, migration, boot handlers, and CI gate must ship together. The app is broken (blank screen) without all boot-blocking handlers; the adapter is useless without the call-site migration.
- **TypeScript strict**: no `any` types in handlers or adapter; `unknown` with narrowing per repo style. `ProfileSummary` must be exported from `types/library.ts` before typed profile handlers can compile.
- **WebKitGTK parity**: every UI change must be re-verified in `./scripts/dev-native.sh` (no `--browser`) before merge. Chrome-only fixes that break WebKitGTK are the primary regression risk.
- **`convertFileSrc` synchrony**: unlike `invoke`/`listen`, `convertFileSrc` is synchronous and cannot be async-wrapped. It must be imported directly from `lib/plugin-stubs/convertFileSrc.ts` at the two call sites (`hooks/useGameCoverArt.ts`, `components/profile-sections/MediaSection.tsx`) — it is NOT covered by Vite alias because aliasing all of `@tauri-apps/api/core` would break `invoke`.
- **`ProfileSummary` not yet exported**: currently a local interface in `hooks/useLibrarySummaries.ts` lines 6-12. Must be moved to `src/crosshook-native/src/types/library.ts` as a Phase 1 prerequisite — blocks typed mock handlers for `profile_list_summaries`.
- **Promise latch in `ensureMocks()`**: `PreferencesContext` fires 3 parallel `callCommand` calls on mount. Without the latch (`let mocksPromise: Promise<...> | null = null`), all three initiate concurrent `import('./mocks')` calls. The latch ensures the first call owns the promise and subsequent calls reuse it.
- **`DevModeChip` must render above `<ProfileProvider>`**: rendering inside a provider means the chip disappears if the provider throws. Placement in `App.tsx` root, outside all providers, is required.
- **Fixture data module-scope init**: `lib/mocks/index.ts` must resolve `URLSearchParams(window.location.search)` at module scope (before `registerMocks()` is called) so fixture state is determined before React mounts and the first `callCommand` fires.

---

## Key Recommendations

- Land `lib/runtime.ts`, `lib/ipc.ts`, `lib/events.ts` first (adapter foundation — everything else imports from these)
- Then plugin stubs in parallel (`dialog`, `shell`, `fs`, `convertFileSrc` are independent of each other)
- Then mock registry skeleton (`mocks/index.ts`, `mocks/store.ts`, `mocks/eventBus.ts`) in parallel with `DevModeChip.tsx` + `dev-indicator.css`
- Then boot-critical handlers (`handlers/settings.ts` + `handlers/profile.ts`) — depend on `store.ts`
- Then mechanical call-site migration (84 `invoke` + 16 `listen` + plugin import rewrites) — depends on adapter + stubs existing
- Then `App.tsx` wiring + `tsconfig.json` + `vite.config.ts` + `package.json` + `scripts/dev-native.sh` config changes — coordinate to avoid conflicts
- Then CI gate (`release.yml` `verify:no-mocks` step) + `AGENTS.md` docs update
- Phase 1 validation: `cargo test` passes; manual smoke of all 9 routes in both browser and Tauri modes; `verify:no-mocks` grep passes on a local `./scripts/build-native.sh --binary-only` before relying on CI
- Phase 2 handler fan-out: each domain PR is independent; suggested priority order is Profiles → Launch (event bus) → Health → Onboarding → Install → Update → Proton → ProtonDB → ProtonUp → Community → Launcher → Discovery → remaining
- Phase 3 fixture variants + debug toggles (`?fixture=`, `?errors=`, `?delay=`, `?onboarding=`) are built on the Phase 2 handler foundation; `?fixture=empty|error|loading` requires enough handler coverage to be meaningful
