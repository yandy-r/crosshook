# dev-web-frontend

CrossHook's React frontend cannot boot at `http://localhost:5173` today because **42 files** import `invoke` from `@tauri-apps/api/core`, **13 files** import `listen` from `@tauri-apps/api/event`, and **6 files** import `@tauri-apps/plugin-*` packages — none of which resolve outside a Tauri WebView. This feature introduces an owned IPC adapter layer under `src/crosshook-native/src/lib/` (`runtime.ts`, `ipc.ts`, `events.ts`, `plugin-stubs/`, `mocks/`) that branches on `isTauri()` at runtime and on a `__WEB_DEV_MODE__` Vite `define` constant at build time, plus a `./scripts/dev-native.sh --browser` flag that starts `vite --mode webdev` on loopback. Every `invoke(` / `listen(` call site migrates mechanically to `callCommand(` / `subscribeEvent(`, plugin imports rewrite to local stubs, and a CI grep sentinel on `dist/assets/*.js` (strings `[dev-mock]`, `getMockRegistry`, `registerMocks`, `MOCK MODE`) is the authoritative guarantee that no mock code ever reaches the production AppImage. Zero new npm dependencies; all runtime state is ephemeral; persistence boundaries are unchanged.

## Relevant Files

- src/crosshook-native/src/App.tsx: Root shell; line 5 imports `listen` from `@tauri-apps/api/event`, line 67 subscribes to `onboarding-check` — needs `subscribeEvent` migration and `__WEB_DEV_MODE__`-gated `<DevModeChip />` render + `.crosshook-app--webdev` className
- src/crosshook-native/src/main.tsx: React entry; no direct IPC usage, but is the eager-import point for `lib/plugin-stubs/convertFileSrc.ts` because `convertFileSrc` is synchronous
- src/crosshook-native/src/vite-env.d.ts: Environment type declarations; must add `declare const __WEB_DEV_MODE__: boolean;`
- src/crosshook-native/vite.config.ts: Vite config; must add mode-conditional `define: { __WEB_DEV_MODE__: mode === 'webdev' }`, `resolve.alias: { '@': './src' }`, and webdev-mode `server.host = '127.0.0.1'` + `strictPort = true`
- src/crosshook-native/package.json: Scripts; must add `"dev:browser": "vite --mode webdev"` — the `--mode webdev` flag is mandatory, plain `vite` breaks the adapter (documented footgun in research-security.md W-1)
- src/crosshook-native/tsconfig.json: TypeScript config; must add `"paths": { "@/*": ["./src/*"] }` to match Vite alias
- scripts/dev-native.sh: Dev launcher; must add `--browser` / `--web` case branch that `cd`s into `NATIVE_DIR` and `exec npm run dev:browser` — no `cargo`, no `tauri` invocation in this branch
- .github/workflows/release.yml: Release CI; must add `verify:no-mocks` step after "Build native AppImage" — greps `dist/assets/*.js` for sentinel strings and fails the build on any hit
- src/crosshook-native/src/context/PreferencesContext.tsx: Most critical IPC migration target; lines 43-46 run a parallel 3-command boot (`settings_load`, `recent_files_load`, `default_steam_client_install_path`) that must resolve from mock handlers before the shell renders
- src/crosshook-native/src/hooks/useLibrarySummaries.ts: Calls `invoke<ProfileSummary[]>('profile_list_summaries')`; `ProfileSummary` type is currently local at lines 6-12 and must be exported from `src/crosshook-native/src/types/library.ts` so mock handlers can reuse it without duplication
- src/crosshook-native/src/utils/optimization-catalog.ts: Non-hook, non-component `invoke` call site; must migrate to `callCommand` like everything else
- src/crosshook-native/src/types/index.ts: Barrel re-export of all TS types; mock handlers will import from here
- src/crosshook-native/src/types/settings.ts: `DEFAULT_APP_SETTINGS`, `AppSettingsData`, `toSettingsSaveRequest` — reused as-is by `handlers/settings.ts`
- src/crosshook-native/src/types/profile.ts: `Profile`, `SerializedGameProfile`, `createDefaultProfile`, `normalizeSerializedGameProfile` — reused as-is by `handlers/profile.ts`
- src/crosshook-native/src/types/library.ts: Target for the `ProfileSummary` export (currently local to `useLibrarySummaries.ts`)
- src/crosshook-native/src/types/launch.ts: `LaunchResult` and launch event payload types consumed by `handlers/launch.ts` in Phase 2
- src/crosshook-native/src/types/install.ts: `InstallStatus` and install event payload types for `handlers/install.ts`
- src/crosshook-native/src/types/health.ts: `EnrichedHealthSummary`, `CachedHealthSnapshot[]` for `handlers/health.ts`
- src/crosshook-native/src/types/proton.ts: `ProtonInstallOption[]` for `handlers/proton.ts`
- src/crosshook-native/src/types/onboarding.ts: `ReadinessCheckResult`, `OnboardingCheckPayload` for `handlers/onboarding.ts` and `App.tsx` listen migration
- src/crosshook-native/src/hooks/useScrollEnhance.ts: CLAUDE.md-mandated scroll-container registry — any new `overflow-y: auto` container must be added to the `SCROLLABLE` selector, relevant if the `<DevModeChip />` or any mock-mode panels introduce a scroll region
- src/crosshook-native/src/styles/variables.css: Design-token source of truth; `--crosshook-color-warning` is the chip/outline accent per UX research two-layer indicator spec
- src/crosshook-native/src/components/pages: All 9 page components that will render in both modes; reference for the "identical component tree" business rule BR-3
- AGENTS.md: Commands reference block; must gain `./scripts/dev-native.sh --browser` entry + loopback-only note
- CLAUDE.md: Repo policy; persistence boundary rules and `docs(internal):` commit prefix apply — no storage additions, but the research/plan commits go under `docs(internal):`
- docs/plans/dev-web-frontend/feature-spec.md: 819-line pre-synthesized feature specification from prior `ycc:feature-research` run — single source of truth for this plan

## Relevant Patterns

**IPC Adapter (Strategy + Runtime Branch)**: Single owned boundary module `callCommand<T>(name, args)` probes `isTauri()` and dispatches to either `@tauri-apps/api/core.invoke` (real) or an in-process `Map<string, Handler>` registry (mock). The adapter throws loudly on unhandled commands. See spec code block at [docs/plans/dev-web-frontend/feature-spec.md](feature-spec.md) lines 261-297.

**Event Bus (In-Process Pub/Sub)**: `subscribeEvent<T>(name, handler)` in browser mode stores listeners in a module-scope `Map<string, Set<Listener>>` and returns a real unsubscribe function; `emitMockEvent(name, payload)` fans out to subscribers. Satisfies the memory-leak avoidance business rule BR-7. See [docs/plans/dev-web-frontend/feature-spec.md](feature-spec.md) lines 301-326.

**Build-Time Dead-Code Branch**: `declare const __WEB_DEV_MODE__: boolean` plus `if (!__WEB_DEV_MODE__) throw` inside `ensureMocks()` lets Rollup's dead-code elimination drop the entire `./mocks` subtree — paired with dynamic `import('./mocks')` so the chunk graph is never emitted in production. This is how `if (false)` is mechanically eliminated by Vite/Rolldown. See [docs/plans/dev-web-frontend/research-technical.md](research-technical.md) and security finding W-1 in [docs/plans/dev-web-frontend/research-security.md](research-security.md).

**Mock Registry Fan-In**: Each domain handler file (`handlers/settings.ts`, `handlers/profile.ts`, …) exports a `register*(map: Map<string, Handler>)` function that mutates the shared map. `lib/mocks/index.ts` orchestrates all `register*` calls in one place. No DSL, no factory — plain map population until Rule of Three triggers a refactor. See example at [docs/plans/dev-web-frontend/feature-spec.md](feature-spec.md) lines 349-373.

**In-Memory MockStore Singleton**: `getStore()` returns a module-scope mutable object keyed by domain (`settings`, `profiles`, `recentFiles`, …). Mutating handlers update the store and return the saved payload to match optimistic-UI re-reads; HMR on a handler file resets the store, which is intentional. See [docs/plans/dev-web-frontend/feature-spec.md](feature-spec.md) lines 222-235.

**Plugin Stub Re-Export / Throw-or-Null**: `lib/plugin-stubs/dialog.ts` re-exports real `@tauri-apps/plugin-dialog` in Tauri mode and returns `null` + `console.warn('[dev-mock] dialog suppressed')` in browser. `shell.ts` / `fs.ts` **throw** on destructive operations (`shell.execute`, `fs.writeFile`, `fs.rename`) — silent no-ops are banned per business rule BR-8. `convertFileSrc.ts` uses a synchronous passthrough in browser. See [docs/plans/dev-web-frontend/feature-spec.md](feature-spec.md) lines 385-391 and decision D4 (lines 792).

**Two-Layer Dev-Mode Indicator**: Layer 1 = `.crosshook-app--webdev { box-shadow: inset 0 0 0 3px var(--crosshook-color-warning) }` on the root shell (zero layout impact, screenshot-crop-resistant). Layer 2 = `<DevModeChip />` fixed-position corner chip reusing existing `crosshook-status-chip--warning` tokens with `role="status"` and `aria-label`. No dismiss button — required by UX research A-6 to prevent accidental mode confusion. See [docs/plans/dev-web-frontend/research-ux.md](research-ux.md) two-layer section.

**URL-Query Fixture Switcher**: `?fixture=populated|empty|error|loading` plus orthogonal `?errors=true`, `?delay=<ms>`, `?onboarding=show` debug toggles, resolved at module init from `window.location.search` and dispatched to fixture-state-aware handlers. Phase 3 scope; Phase 1 only needs `populated`. See [docs/plans/dev-web-frontend/feature-spec.md](feature-spec.md) lines 541-542.

**CI Grep Sentinel (Authoritative Control)**: `.github/workflows/release.yml` runs `grep -rl '\[dev-mock\]\|getMockRegistry\|registerMocks\|MOCK MODE' src/crosshook-native/dist/assets/*.js` after AppImage build and fails on any hit. The `__WEB_DEV_MODE__` define is the primary control; this grep is the fail-safe per security finding W-1. See [docs/plans/dev-web-frontend/research-security.md](research-security.md).

## Relevant Docs

**docs/plans/dev-web-frontend/feature-spec.md**: You _must_ read this when designing any task in this plan — it is the 819-line pre-synthesized feature specification containing the complete architecture diagram, all 14 success criteria (SC-1 through SC-14), all 12 business rules (BR-1 through BR-12), all 6 resolved decisions (D1 through D6), full file inventory, and the 3-phase task breakdown preview. Treat resolved decisions as settled — do not re-litigate them.

**docs/plans/dev-web-frontend/research-technical.md**: You _must_ read this when implementing `lib/ipc.ts`, `lib/events.ts`, `lib/plugin-stubs/convertFileSrc.ts`, or `vite.config.ts` — it contains per-module implementation snippets, the `convertFileSrc` synchrony edge case, the boot sequence analysis, the tree-shaking strategy rationale, and the `vite.config.ts` assessment.

**docs/plans/dev-web-frontend/research-business.md**: You _must_ read this when extending the mock handler catalog — it contains the full IPC call inventory grouped by domain, the 8 user stories, the 12 business rules, the 4 architectural decisions (AD-1 through AD-4), and the storage boundary classification confirming every piece of state is runtime-only.

**docs/plans/dev-web-frontend/research-security.md**: You _must_ read this before touching `vite.config.ts`, `release.yml`, or any fixture file — it enumerates the 3 CRITICAL findings (production leak, LAN exposure, fixture secret leakage), 3 WARNING findings (W-1 tree-shaking, W-2 dev-server host, W-3 fixture PII), and 6 advisories (A-1 through A-6) with required mitigations.

**docs/plans/dev-web-frontend/research-ux.md**: You _must_ read this when implementing the `<DevModeChip />` component, `dev-indicator.css`, or any fixture-switching UX — it contains the two-layer dev-mode indicator spec, sidebar layout analysis, Storybook/MSW/Vercel pattern review, and the browser-vs-Tauri parity gotcha list (scrollbars, `color-mix()`, `useScrollEnhance` selectors).

**docs/plans/dev-web-frontend/research-practices.md**: You _must_ read this when performing the mechanical call-site migration — it documents the 84/42-call-site migration scope (the count drifted upward since research), the existing reusable code table, 4-module modularity design, rule-of-three verdicts, and the 13 components that call `invoke()` directly (architectural smell, tracked as a follow-up `refactor:` issue out of scope for this plan).

**docs/plans/dev-web-frontend/research-external.md**: You _must_ read this when evaluating any alternative library proposal — it contains the full `mockIPC` vs hand-rolled `callCommand` trade-off table, the MSW rejection rationale, the `@faker-js/faker` sabotage precedent, and the verified correction that `@tauri-apps/api/mocks.mockIPC` does work in a plain browser (the original research was wrong) but is still rejected for surface-coverage reasons.

**docs/plans/dev-web-frontend/research-recommendations.md**: You _must_ read this when sequencing work — it contains the full recommendation synthesis, the 3-phase task breakdown with ordering rationale, the 6-entry alternative approaches table, and the 9-item decision-needed list (now resolved as D1-D6 in feature-spec.md).

**CLAUDE.md**: You _must_ read this before any implementation commit — it specifies the `docs(internal):` commit prefix for `docs/plans/**` changes, the persistence boundary classification rules (this feature is runtime-only and adds nothing to the boundary, which must be stated explicitly in PR descriptions), the issue-template policy, and the WebKitGTK `useScrollEnhance` scroll-container registry rule that affects any new scrollable UI.

**AGENTS.md**: You _must_ read this when updating documentation at the end of Phase 1 — it contains the Commands short-reference block that gains the new `./scripts/dev-native.sh --browser` entry, the loopback-only binding note, and the test verification command (`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`) that must continue to pass after the migration.
