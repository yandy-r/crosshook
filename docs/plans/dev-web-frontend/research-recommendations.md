# dev-web-frontend — Recommendations & Risk Synthesis

> Synthesis output for the `dev-web-frontend` feature: a `./scripts/dev-native.sh --browser` mode that runs **only** the Vite frontend in a normal browser at `http://localhost:5173`, with mock data substituted for Tauri IPC, strictly to accelerate UI/UX iteration (DevTools, logs, element inspection).
> **Revision note (post practices-researcher round 2 + tech-designer consult):** This synthesis has been revised twice. Key changes from the original draft: (1) recommendation is a hand-rolled `lib/ipc.ts` adapter, not `@tauri-apps/api/mocks` mockIPC; (2) file grouping is per-feature-area (~7 files), not per-command or per-hook (~18 files); (3) Phase 1 MVP scope is defined by the 10-command "boot-blocking" set, not arbitrary handler coverage; (4) call-site numbers are corrected to practices-researcher's verified count (88 distinct commands / ~84+ call sites / 42 files). One technical claim about `mockIPC` from practices-researcher was verified incorrect against upstream source and is documented inline so future readers don't rely on a wrong rejection reason.

---

## Executive Summary

CrossHook's frontend currently makes **~88 distinct `invoke()` commands** across **~84 call sites** (138 `invoke[<(]` occurrences across 42 files when including type imports), plus **~14 distinct Tauri events** (`profiles-changed`, `launch-log`, `update-log`, `launch-diagnostic`, `onboarding-check`, etc.), **3 Tauri plugins** (`@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-shell`, `@tauri-apps/plugin-fs`), and the `convertFileSrc` art helper used by `useGameCoverArt` and `MediaSection`. Running this UI in a vanilla browser without IPC requires a thin, well-structured mock layer — but **not a heavyweight HTTP-mocking framework, not Storybook, and not a parallel build system**.

**Recommended approach:** A dev-only IPC adapter under `src/crosshook-native/src/lib/`, per practices-researcher's verdict (seconded by tech-designer's Option A). The shape:

1. **`lib/runtime.ts`** — `isTauri()` probe checking `window.__TAURI_INTERNALS__` (~6 lines)
2. **`lib/ipc.ts`** — `callCommand<T>` wrapper that routes to real `invoke` or the dynamic mock registry (~15 lines)
3. **`lib/events.ts`** — `subscribeEvent`/`emitEvent` shim that routes to real `listen` or an in-process bus
4. **`lib/plugin-stubs/`** — `dialog.ts`, `shell.ts`, `fs.ts`, `convertFileSrc.ts` that re-export the real plugins in Tauri mode and loud-warning no-ops in browser mode
5. **`lib/mocks/`** — **per-feature-area** handler files (not per-hook, not per-command): ~7 files matching the existing `src/types/` domain structure (settings, profiles, library, launch, health, community, system)
6. **Mechanical migration** of all 84 `invoke(` call sites to `callCommand(` and all 16 `listen(` call sites to `subscribeEvent(`
7. **`DevModeBanner`** component — fixed top stripe, plain inline-styled `<div>` in `App.tsx` above `<ProfileProvider>` (following the existing `RouteBanner` / `OfflineStatusBadge` pattern)
8. **Vite dead-code elimination** of the entire mock subtree in production builds via the dynamic-import gate

**MVP scope defined by boot-blocking commands.** Phase 1 must mock the 10 commands that fire before the app shell can render (settings, preferences, profile lists, onboarding check, optimization catalog, batch health validation). Anything else throws a visible `[dev-mock] No mock registered for command: ...` error until a contributor adds it.

**Critical risks** (full detail in [Risk Assessment](#risk-assessment)): security posture of a browser-exposed dev server, drift between mocks and the real Rust IPC contract, accidental shipment of mocks to AppImage, and the maintenance tax across ~88 commands. None are blocking; all are mitigable.

> **Note on a contested technical claim:** practices-researcher asserted (twice, across two messages) that `@tauri-apps/api/mocks` `mockIPC` "requires the Tauri bridge to exist", "patches `window.__TAURI_INTERNALS__.ipc.postMessage`", and is "unusable in pure browser". **This is incorrect.** Verified twice against upstream `tauri-apps/tauri:packages/api/src/mocks.ts`:
>
> ```ts
> function mockInternals() {
>   window.__TAURI_INTERNALS__ = window.__TAURI_INTERNALS__ ?? {};
>   window.__TAURI_EVENT_PLUGIN_INTERNALS__ = window.__TAURI_EVENT_PLUGIN_INTERNALS__ ?? {};
> }
> ```
>
> `mockIPC` calls `mockInternals()` which creates `__TAURI_INTERNALS__` if absent, then replaces `__TAURI_INTERNALS__.invoke` with a custom async function. It does not patch `ipc.postMessage`. It works in any JavaScript environment, including plain browsers and Node.js SSG contexts (as documented in Tauri's own usage notes).
>
> **This correction does not change the final recommendation.** The hand-rolled `lib/ipc.ts` adapter is still the right choice for CrossHook, but the rejection reasons for `mockIPC` are **(a) tree-shake provability** (we own greppable sentinel strings for a CI guard), **(b) coverage surface** (it handles `invoke` only — not `listen`, the 3 plugins, or `convertFileSrc`), and **(c) distinctive error messages**, _not_ "it's broken in browsers". Intellectual honesty matters here: if a future contributor runs `mockIPC` in a browser and watches it work, they should not conclude the whole analysis was sloppy.

---

## Implementation Recommendations

### Recommended Approach

**A hand-rolled `lib/ipc.ts` adapter + per-feature-area mock modules + plugin stubs.**

#### Why this combination

- **Single owned boundary**. One `callCommand<T>` import lives in every call site. The adapter routes to either the real `invoke` from `@tauri-apps/api/core` or the local mock registry. The same pattern repeats for `subscribeEvent` (listen) and the three plugin packages.
- **Covers everything `mockIPC` would not**. Tauri's official mock covers `invoke` only. CrossHook's frontend also uses `listen()` (16 call sites), three plugin packages, and `convertFileSrc`. The hand-rolled adapter handles all of these in one `lib/` directory.
- **Tree-shake provability (the deciding factor)**. The mock subtree lives in files we own, loaded via `dynamic import('./mocks')`. We can add a CI smoke test that greps the dist bundle for `lib/mocks`, `registerMocks`, and `MOCK MODE` sentinel strings and fails the build on any hit. `mockIPC` lives in `node_modules` with Tauri-controlled symbols we can't reliably gate against.
- **No new dependencies**. The adapter is ~30 lines of TypeScript. The mock registry is per-feature-area handler files, all consuming existing types from `src/crosshook-native/src/types/*.ts`.
- **Distinctive error messages**. Missing-mock errors don't look like real Tauri errors — `[dev-mock] No mock registered for command: profile_xyz` is visibly different from a real IPC failure, which saves debugging time during onboarding.
- **Forces a useful side-effect**: every IPC call site is now visible at the `lib/ipc.ts` boundary, making future refactors (centralized error handling, per-call telemetry, stricter typing, contract testing) trivial. Tech-designer's reframing: "Option A is a maintainability win even if mocks weren't a goal; the browser-dev feature is the forcing function we needed to justify the small refactor."
- **Mechanical migration**: the migration is `invoke(` → `callCommand(` find-and-replace across 84 call sites in 42 files. No semantic changes. A reviewer can skim it in ~15 minutes because every change has the same shape.

#### Recommended file layout

```
src/crosshook-native/src/lib/
├── runtime.ts                    # isTauri() probe (6 lines, zero deps)
├── ipc.ts                        # callCommand<T>(name, args) adapter
├── events.ts                     # subscribeEvent<T>(name, handler) adapter + in-process bus
├── DevModeBanner.tsx             # Fixed top stripe, plain inline-styled div, rendered in App.tsx
├── plugin-stubs/
│   ├── dialog.ts                 # Re-exports @tauri-apps/plugin-dialog in Tauri mode; loud-warning stubs in browser
│   ├── shell.ts                  # Same pattern for shell
│   ├── fs.ts                     # Same pattern for fs
│   └── convertFileSrc.ts         # Real convertFileSrc in Tauri mode; placeholder data URL in browser
└── mocks/
    ├── index.ts                  # registerMocks(): merges all per-area handler modules
    ├── store.ts                  # In-memory Map for session round-trip state (if needed)
    ├── eventBus.ts               # In-process pub/sub wired to lib/events.ts
    ├── README.md                 # How to add a handler (with one worked example)
    ├── settings.ts               # settings_load, recent_files_*, default_steam_client_install_path
    ├── profiles.ts               # profile_list, profile_load, profile_save, profile_duplicate, profile_rename, list_summaries, list_favorites, export_toml, optimization preset commands
    ├── library.ts                # fetch_game_cover_art, import_custom_art, fetch_game_metadata, auto_populate_steam, build_steam_launch_options_command
    ├── launch.ts                 # launch_game, launch_trainer, preview_launch, validate_launch, check_game_running, verify_trainer_hash, check_gamescope_session, run_executable family, launch events
    ├── health.ts                 # batch_validate_profiles, get_profile_health, get_cached_health_snapshots, check_offline_readiness family, protondb_*, health events
    ├── community.ts              # community_list_*, community_sync, community_add_tap, community_prepare_import, community_export_profile, discovery_search_trainers
    └── system.ts                 # get_optimization_catalog, get_trainer_type_catalog, get_mangohud_presets, check_readiness, dismiss_onboarding, check_version_status, list_proton_installs, check_proton_migrations, apply_*, protonup_*, get_dependency_status, prefix_storage family, list_launchers, launcher export family, install_game, validate_install_request, install_default_prefix_path, update_game, validate_update_request, cancel_update, export_diagnostics, detect_protontricks_binary
```

**7 handler files** — matching the existing `src/types/` domain structure. Per practices-researcher: "With 88 unique command names, one file per command is absurd. The right grouping follows the existing `src/types/` domain structure. That's 5–7 files covering all domains."

The `system.ts` file is the catch-all for commands that don't naturally fit library/launch/health/community — it covers catalogs, onboarding, Proton stack management, prefix storage, launcher export, install/update flows, and diagnostics. If `system.ts` ever grows past ~400 lines in practice, split it then — not speculatively now.

Each module exports a `register(map: Map<string, Handler>)` function that the `mocks/index.ts` calls in sequence to populate the central command-to-handler map.

#### Wiring at the boundary

`src/crosshook-native/src/lib/ipc.ts`:

```ts
import type { InvokeArgs } from '@tauri-apps/api/core';
import { isTauri } from './runtime';

type Handler = (args: unknown) => unknown | Promise<unknown>;

let mockMap: Map<string, Handler> | null = null;

async function ensureMocks(): Promise<Map<string, Handler>> {
  if (mockMap) return mockMap;
  const { registerMocks } = await import('./mocks');
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
      `[dev-mock] No mock registered for command: ${name}. Add a handler in src/lib/mocks/<area>.ts — see lib/mocks/README.md`
    );
  }
  if (import.meta.env.DEV) {
    console.debug('[dev-mock] callCommand', name, args);
  }
  return handler(args ?? {}) as Promise<T>;
}
```

`src/crosshook-native/src/lib/runtime.ts`:

```ts
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
```

`src/crosshook-native/src/lib/events.ts`:

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

The dynamic `import('./mocks')` and dynamic `import('@tauri-apps/api/core')` are both load-bearing — Vite will tree-shake the entire mock subtree out of the production bundle because it is only reachable via the `!isTauri()` branch, and the production AppImage runs inside Tauri so the branch is provably dead at runtime.

#### Script flag

`./scripts/dev-native.sh --browser` becomes a thin branch:

```bash
case "${1:-}" in
  --browser|--web)
    cd "$NATIVE_DIR"
    [[ -x "$NATIVE_DIR/node_modules/.bin/vite" ]] || npm ci
    echo "Starting CrossHook frontend-only dev server (browser mock IPC)..."
    echo "  -> http://localhost:5173"
    exec npm run dev
    ;;
  ...
esac
```

`npm run dev` already maps to `vite` in `package.json`. Adding `dev:browser` as a semantic alias is recommended.

### Phase 1 MVP — The 10 Boot-Blocking Commands

Per practices-researcher's boot-sequence analysis: the app initializes with a cascade of IPC calls. `settings_load` + `recent_files_load` + `default_steam_client_install_path` fire simultaneously in `PreferencesContext`, then `profile_list` fires in `useProfile`, then `profile_list_summaries` fires in `useLibrarySummaries`. If any one of these hits an unmocked command and throws, the entire app shell fails to render — a blank screen, not a partially-rendered UI.

**Phase 1 must mock these 10 commands** or the app doesn't render at all:

| #   | Command                             | Called from                                   | Phase |
| --- | ----------------------------------- | --------------------------------------------- | ----- |
| 1   | `settings_load`                     | `PreferencesContext` (boot)                   | 1     |
| 2   | `recent_files_load`                 | `PreferencesContext` (boot)                   | 1     |
| 3   | `recent_files_save`                 | `PreferencesContext` (boot)                   | 1     |
| 4   | `default_steam_client_install_path` | `PreferencesContext` (boot)                   | 1     |
| 5   | `profile_list`                      | `useProfile` (initial load)                   | 1     |
| 6   | `profile_list_favorites`            | `useProfile` (initial load)                   | 1     |
| 7   | `profile_list_summaries`            | `useLibrarySummaries` (Library page)          | 1     |
| 8   | `check_readiness`                   | `useOnboarding` (post-init)                   | 1     |
| 9   | `get_optimization_catalog`          | `useLaunchOptimizationCatalog` (profile load) | 1     |
| 10  | `batch_validate_profiles`           | `useOnboarding` (post-init)                   | 1     |

With these 10 mocked, the app shell renders and the Library page is navigable. Everything else in Phase 2 is **on-demand** — a contributor who opens the Launch tab and hits an unmocked command sees the distinctive error, adds the handler, iterates. No need to front-load coverage for screens nobody is actively working on.

### Quick Wins

1. **Reuse existing fixtures**. `DEFAULT_APP_SETTINGS` and `createDefaultProfile()` already exist in the type files. The settings and profile handlers should call them directly rather than re-creating sample data.
2. **Reuse existing types**. Every handler imports return types from `src/crosshook-native/src/types/*.ts`. The TS compiler becomes the contract checker — if a Rust command's response shape changes and the TS types are regenerated/updated, mock handlers immediately fail to compile.
3. **Default-error fallback**. Already in the `callCommand` implementation above. The error message points at `src/lib/mocks/<area>.ts` so contributors know exactly where to add the missing handler.
4. **Plugin stubs must fail loud**. `@tauri-apps/plugin-dialog` is used for file pickers in 6 places. A dialog stub that returns `null` synchronously would silently mimic "user cancelled" when in reality no dialog appeared. Stubs should `console.warn` AND surface a visible toast saying "Mock mode: file dialog suppressed".
5. **`convertFileSrc` placeholder**. `useGameCoverArt` and `MediaSection` call `convertFileSrc` to render local cover art. In browser mode, return a static placeholder image so cover art renders something instead of a broken image icon.
6. **Console-log all mock calls**. `console.debug('[dev-mock] callCommand', name, args)` is invaluable for contributors. Stripped in production via the dead-code branch.
7. **Document one example handler in `lib/mocks/README.md`**. New handlers become a 5-minute copy-paste job. Single biggest determinant of long-term maintenance cost.
8. **Validate WebKitGTK scroll conventions in browsers** as a side benefit. Chrome/Firefox handle scroll differently from WebKitGTK, so contributors get free sanity-checks on `useScrollEnhance` selectors.

### Phasing

**Phase 1 — Foundation (single PR, mergeable in isolation)**

Goal: A contributor can run `./scripts/dev-native.sh --browser`, see the app boot in a browser at `localhost:5173`, see the dev banner, and navigate the Library page without errors. Launch/Health/Community/etc. tabs throw visible `[dev-mock]` errors until a contributor claims them.

Ships: script flag + Vite config hardening + `lib/` adapter scaffold + `lib/events.ts` shim + plugin stubs + DevModeBanner + mechanical migration of all 84 `invoke(` + 16 `listen(` + plugin imports + `lib/mocks/settings.ts` + `lib/mocks/profiles.ts` + minimal boot-blocking handlers in `lib/mocks/system.ts` + CI grep guard + `AGENTS.md` update.

**Phase 2 — On-demand handler coverage**

Each sub-phase can be its own PR, claimed when a contributor actually iterates on that area. No front-loading.

Order by likely iteration value (but not prescriptive):

1. **Launch** — most active iteration area; includes events (`launch-log`, `launch-diagnostic`, `launch-complete`)
2. **Health Dashboard** — large complex page (57k LOC); benefits enormously from browser DevTools
3. **Install / Update flows** — multi-step wizards with event streaming
4. **Community / Discovery / ProtonDB / ProtonUp** — networked features, ideal mock targets
5. **Launcher Export** — modal-heavy flow
6. **Proton stack management** — `list_proton_installs`, migrations
7. **Remaining `system.ts` additions** — prefix storage, diagnostics, catalog, art, steam — added as needed

**Phase 3 — Polish**

- Fixture variants (empty / loading / error / loaded scenarios) selectable via `?scenario=` URL query param
- Optional: handler-coverage check script (`pnpm dev:browser:check`) that scans `commands/*.rs` and lists unmocked commands
- Document the dev-browser flow in the project README
- Optional: Playwright smoke test that boots the dev server and screenshot-checks every route

---

## Improvement Ideas

These are adjacent enhancements worth tracking but **not** in scope for the initial feature:

1. **Fixture variant switcher** — `?scenario=empty|loading|error|populated` URL param. Hot-swap without re-editing files. Cheap, deferrable.
2. **Visual regression baseline** — Once `dev-web-frontend` exists, Playwright can screenshot the UI in a real browser without needing a Tauri runtime. This unlocks a future visual-regression suite that is currently impossible.
3. **Storybook-lite entrypoint** — A second Vite entrypoint at `/components` that renders a directory of components in isolation. **Only** if contributors actually want it; do not build speculatively. Per practices-researcher: the context-dependency (`ProfileContext`/`PreferencesContext`/`LaunchStateContext` interleaved) means Storybook decorators would be a substantial project of their own.
4. **a11y audit pass** — Browser DevTools axe extensions become trivially usable once browser mode lands.
5. **Mock IPC introspection panel** — A floating dev panel logging every `callCommand` call with request/response shapes. Phase 4+ at earliest.
6. **Round-trip persistence to localStorage** — Currently the in-memory store resets on reload. Only add if anyone asks. Cross-reference security-researcher's findings on browser storage scope before doing so.
7. **Contract test against the real Rust `#[tauri::command]` registry** — Generate the canonical command list at build time, diff against registered handlers, fail CI on drift. High value, medium effort. Strong candidate for follow-up once handler coverage stabilizes.
8. **MSW for the few real HTTP calls** — **Reject permanently.** Per practices-researcher: the HTTP calls happen on the Rust side (`invoke('protondb_lookup', ...)`, `invoke('community_sync')`), not the frontend. There is nothing for MSW to intercept. Do not revisit.
9. **Fix the 13 components calling `invoke` directly** — practices-researcher flags this as a mild architectural smell (components bypassing hooks). The browser-mode migration is the perfect time to convert them to hooks, but it's strictly out of scope for Phase 1. Track as a follow-up `refactor:` issue.
10. **Export `ProfileSummary` from `types/library.ts`** — practices-researcher notes this type is not exported. Fix as part of Phase 1 when writing `lib/mocks/profiles.ts`.

---

## Risk Assessment

> Security findings tagged `[CRITICAL]` MUST be addressed before this feature ships, even as a dev-only convenience.

### CRITICAL

> **Note:** the security-researcher's full report should be cross-referenced when available. The findings below are derived from first-principles analysis plus practices-researcher's gap analysis.

- **`[CRITICAL]` Vite dev server must NOT be exposed beyond loopback.** Vite's default is `127.0.0.1` but contributors may run with `--host` for LAN testing. The CrossHook UI loads real Steam library paths, ProtonDB lookups, profile data, and prefix paths — anything broadcast on a LAN is exposed. **Mitigation:** explicitly set `server.host = '127.0.0.1'` and `server.strictPort = true` in `vite.config.ts` and document that `--host 0.0.0.0` is unsupported in browser mode. Reject `--host` overrides at the script level if at all possible.
- **`[CRITICAL]` Production AppImage MUST NOT contain mock code.** A bundled mock could silently mask real IPC failures, or a mistake in runtime detection could cause a release to silently swallow real backend errors. **Mitigation:** (a) dynamic `import('./mocks')` so Vite tree-shakes the chunk; (b) the `!isTauri()` branch is provably dead at runtime in production builds; (c) add a CI smoke test that greps the dist bundle for `lib/mocks`, `registerMocks`, `MOCK MODE`, and `[dev-mock] callCommand` and fails the build if any are found. **This is the reason we own the sentinel strings — `mockIPC` cannot be gated this way.**
- **`[CRITICAL]` Mock data must not contain real secrets.** Fixtures will be checked into the repo. Any sample profile, sample Steam path, sample API token, sample license key, or sample user identifier must be obviously synthetic (`user@example.com`, `00000000-0000-0000-0000-000000000000`, `STEAM_ID_PLACEHOLDER`). **Mitigation:** PR review checklist on changes touching `lib/mocks/`; consider a CI lint that bans known token/key patterns and high-entropy strings in that directory.

### HIGH

- **Drift between mocks and the real Rust IPC contract.** When a Rust `#[tauri::command]` adds, removes, or renames a parameter or response field, the mock can lull contributors into a false sense of "it works in browser, ship it". **Mitigation (short-term):** force handlers to import from `src/types/*.ts` so TS catches structural drift. **Mitigation (long-term):** Improvement #7 above — generate a canonical command list from Rust and diff against registered handlers in CI.
- **Event-driven flows under-tested.** Many critical UX flows depend on Tauri events, not just `invoke` (`launch-log` streaming, `update-complete`, `profile-health-batch-complete`). Without an event shim, large parts of the UI are unreachable in browser mode. Practices-researcher's note: "`listen()` event stubs silence all event-driven state transitions in browser mode (launch progress, console output, etc.) — acceptable for UI iteration but should be documented". **Mitigation:** the `lib/events.ts` shim with `emitMockEvent` is **MVP, not optional**. Phase 1 ships event support; Phase 2 handlers for launch/update/health MUST call `emitMockEvent` to drive realistic state transitions, not just no-op the listeners. Document the event stubs prominently in the dev banner copy and `lib/mocks/README.md`.
- **Plugin-stub silent no-op risk.** `@tauri-apps/plugin-dialog` is used for file pickers in 6 places. A dialog stub that returns `null` silently tricks the UI into thinking the user cancelled, when in fact they never had a dialog at all. **Mitigation:** dialog stubs `console.warn` loudly AND surface a visible toast ("Mock mode: file dialog suppressed"). Never silent.
- **Boot-sequence fragility.** Per practices-researcher: if any one of the 10 boot-blocking commands throws, the entire app shell fails to render — blank screen, not partial UI. **Mitigation:** all 10 must ship in Phase 1. A test in Phase 3 (Playwright smoke) should verify the app shell renders cleanly in browser mode.
- **Security headers / CSP differ between WebView and browser.** Tauri WebView runs under a tight CSP. Chrome/Firefox don't. A contributor may write code that accidentally relies on browser-only behaviour (e.g. `eval`, inline `<script>`, unsanitised innerHTML). **Mitigation:** Vite dev server should mirror the production CSP via Vite middleware, OR document the limitation and rely on the `tauri dev` smoke test before merge.
- **WebKitGTK quirks not reproducible in browsers.** The reverse problem: scroll behaviour, font rendering, focus rings, and IME handling differ. Contributors may "fix" something that looks broken in Chrome but was actually correct in WebKitGTK. **Mitigation:** `lib/mocks/README.md` makes this explicit; the dev banner copy says "UI must be re-verified in `./scripts/dev-native.sh` before merge".

### MEDIUM

- **Maintenance tax on 88 commands / 84 call sites / 7 handler files.** Every new IPC command added to the Rust side will have to grow a mock counterpart, or contributors will encounter the unhandled-command error. **Mitigation:** make the unhandled-command path informative (already in the example `callCommand` above), and consider a `pnpm dev:browser:check` script in Phase 3 that scans the Rust commands directory and lists missing handlers.
- **Mock state management ambiguity.** Some commands genuinely round-trip within a session (`profile_save` → `profile_list` → `profile_load`). Others are read-only (`get_optimization_catalog`). Mixing them without convention will create bugs. **Mitigation:** document the convention up front — pure-read handlers reach for hardcoded fixtures, mutating commands use `lib/mocks/store.ts` if round-trip is actually needed. Per practices-researcher: "`profile_save` in mock mode returns `undefined` (no-op). If a developer saves a profile edit and then navigates away, the edit is lost on reload. That is fine and expected for UI iteration."
- **First-render flicker.** The dynamic mock import is async, so the initial paint may show empty state before fixtures load. **Mitigation:** the `callCommand` adapter resolves promises only after the dynamic import completes, so React Suspense or initial-loading states will work as expected. No bootstrap-time `await` is required; the async behavior matches Tauri's own.
- **Banner accessibility / layout intrusion.** A fixed banner can overlap routes that compute viewport heights via `100vh`. **Mitigation:** banner must subtract its own height from layout via CSS variable, OR be a non-fixed top stripe inside the layout shell (`crosshook-page-scroll-shell`). Follow the existing `RouteBanner`/`OfflineStatusBadge` pattern.
- **13 components call `invoke()` directly.** Practices-researcher flags this as a smell: components should go through hooks. The browser-mode migration converts them mechanically without fixing the deeper issue. **Mitigation:** track the refactor as a follow-up; do not bundle into Phase 1.

### LOW

- **Bundle size of mock module in dev** — irrelevant in dev, removed in prod. Non-issue.
- **HMR behaviour with mock state** — fixture edits trigger a reload; in-memory store resets. Acceptable.
- **Event bus shim drift from Tauri's real event semantics** — the real API supports per-window scoping; the shim is global. Document the limitation.
- **`convertFileSrc` placeholder visual** — covers will look like placeholders, not real game art. Acceptable for UI iteration.

### Risk Mitigations Summary Table

| Risk                                         | Severity | Primary Mitigation                                                                                          |
| -------------------------------------------- | -------- | ----------------------------------------------------------------------------------------------------------- |
| Vite dev server exposed beyond loopback      | CRITICAL | Force `server.host = '127.0.0.1'`, strict port, doc no-LAN policy                                           |
| Mock code shipped in AppImage                | CRITICAL | Dynamic import + dead-code branch + CI grep for owned sentinel strings                                      |
| Real secrets in fixtures                     | CRITICAL | PR review checklist + CI secret-pattern lint on `lib/mocks/`                                                |
| Mock/Rust contract drift                     | HIGH     | Import shared types; CI command-registry diff (follow-up)                                                   |
| Events not mocked                            | HIGH     | `lib/events.ts` shim is MVP, not optional; Phase 2 handlers must call `emitMockEvent`                       |
| Plugin stubs silently returning `null`       | HIGH     | `console.warn` loudly + visible toast; never silent no-op                                                   |
| Boot-sequence fragility                      | HIGH     | All 10 boot-blocking commands ship in Phase 1                                                               |
| CSP / WebKit quirks differ                   | HIGH     | Mirror prod CSP in dev OR document limitation; banner copy reminds re-verify in `tauri dev`                 |
| 88-command maintenance tax                   | MEDIUM   | Helpful unhandled-command error + doc + handler-coverage check script                                       |
| State management ambiguity                   | MEDIUM   | Convention: pure-reads use hardcoded fixtures, mutations use `lib/mocks/store.ts` only if round-trip needed |
| First-render flicker                         | MEDIUM   | Adapter is async-by-default; React handles loading states naturally                                         |
| Banner layout intrusion                      | MEDIUM   | Follow `RouteBanner`/`OfflineStatusBadge` pattern                                                           |
| Direct `invoke()` from components (13 sites) | MEDIUM   | Track as follow-up refactor; don't bundle into Phase 1                                                      |

---

## Alternative Approaches

| #   | Approach                                                                                | Effort | Pros                                                                                                                                                                                                                                             | Cons                                                                                                                                                                                                                                                                                                                          | Recommendation                                                                                                                                        |
| --- | --------------------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| (a) | **Hand-rolled `lib/ipc.ts` adapter + per-feature-area mock modules + plugin stubs**     | **M**  | Single owned boundary covering `invoke` + `listen` + plugins + `convertFileSrc`; tree-shake provability via owned sentinel strings; zero new deps; mechanical migration; distinctive error messages; future-proof against Tauri sub-export drift | Touches every existing call site (84 sites in 42 files); creates a wrapper layer that has to be maintained forever (mitigated by the layer being trivial and bringing independent maintainability wins)                                                                                                                       | **CHOSEN**                                                                                                                                            |
| (b) | **`@tauri-apps/api/mocks` (`mockIPC`)**                                                 | S      | Zero new deps; official Tauri-team-maintained path; intercepts at the right layer; **does work in plain browsers** (verified against upstream source `tauri-apps/tauri:packages/api/src/mocks.ts`)                                               | **Doesn't cover events, plugin packages, or `convertFileSrc`** — would require a parallel `lib/` adapter anyway for these; **tree-shake not greppable** (mock symbols live in `node_modules`, we can't own sentinel strings for a CI guard); error messages look identical to real Tauri IPC failures, adding onboarding cost | Viable backup; rejected for (1) tree-shake provability, (2) coverage surface, (3) error distinctiveness. _Not_ rejected for being broken in browsers. |
| (c) | **Hybrid adapter wrapping `mockIPC` internally**                                        | M      | Type-safe boundary plus official mock plumbing                                                                                                                                                                                                   | Worst of both worlds: pays for the registry AND keeps the `node_modules` surface area; the 30 LOC custom registry is cheaper than the mental cost of explaining "why are we calling mockIPC from inside a wrapper"                                                                                                            | Reject                                                                                                                                                |
| (d) | **MSW** (Mock Service Worker)                                                           | L      | Battle-tested; has DevTools                                                                                                                                                                                                                      | Tauri IPC is not HTTP; the frontend doesn't make HTTP calls at all (ProtonDB/community go through `invoke` to Rust). There is literally nothing for MSW to intercept.                                                                                                                                                         | **Reject permanently.** Per practices-researcher.                                                                                                     |
| (e) | **Storybook**                                                                           | XL     | Per-component isolation                                                                                                                                                                                                                          | Different feature entirely; doesn't satisfy the user's "run full UI in a browser" goal; heavy context-dependency (`ProfileContext`/`PreferencesContext`/`LaunchStateContext`) would require custom decorators which is a project of its own                                                                                   | Defer; possible future complement                                                                                                                     |
| (f) | **Vite module alias** (alias `@tauri-apps/api/core` to a local mock module in dev only) | M      | No call-site changes                                                                                                                                                                                                                             | Diverges production and dev module graphs; complicates tree-shaking; surprises contributors who expect imports to mean what they say                                                                                                                                                                                          | Reject — too clever, hurts discoverability                                                                                                            |
| (g) | **Build-time codegen of stub responses from Rust types**                                | XL     | Zero hand-written fixtures; perfect contract sync                                                                                                                                                                                                | Massive infrastructure investment; requires Rust → TS pipeline; can't represent meaningful sample data; over-engineered for a dev convenience                                                                                                                                                                                 | Reject for now; revisit if a contract test ever lands                                                                                                 |

### Why approach (a) wins

1. **Tree-shake provability is the deciding factor.** The security-researcher's primary concern is mock code reaching production. Only approach (a) gives us greppable sentinel strings we own. Option (b)'s mocks live in `node_modules` under Tauri-controlled symbols that can be minified to single letters — we cannot write a reliable CI gate against them.
2. **Coverage surface**: `lib/` covers `invoke` AND `listen` AND 3 plugins AND `convertFileSrc` in one directory. Option (b) covers `invoke` only, forcing a parallel `lib/` anyway.
3. **Distinctive error messages**: `[dev-mock] No mock registered for command: X` is visibly different from a real Tauri IPC failure.
4. **Mechanical migration**: practices-researcher and tech-designer both confirm the migration is find-and-replace across 84 call sites. No semantic changes.
5. **Independent maintainability win**: tech-designer's reframing applies — "Option (a) is worth doing even if browser mode weren't a goal. The browser-dev feature is the forcing function we needed to justify the small refactor."
6. **Doesn't depend on a Tauri sub-export**: our dev sandbox doesn't have to track `@tauri-apps/api/mocks` evolution.

The **only** reason to prefer (b) over (a) would be to avoid touching the 84 call sites. The migration is mechanical and brings real benefits (centralized error handling, single import to grep for, etc.), so the call-site churn is a feature not a cost.

---

## Task Breakdown Preview

> Phases and groups for the plan-workflow agent. Each phase is independently reviewable and mergeable. Phase 1 is the MVP foundation; Phase 2 fans out on-demand handler coverage; Phase 3 is polish.

### Phase 1 — Foundation (one PR)

**Goal:** A contributor can run `./scripts/dev-native.sh --browser`, see the app boot in a browser at `localhost:5173`, see the dev banner, and navigate the Library page without errors. All 10 boot-blocking commands are mocked. Other tabs throw visible `[dev-mock]` errors until a contributor claims them.

- **1.1** Add `--browser` (and `--web` alias) flag to `scripts/dev-native.sh` with help text update
- **1.2** Add `dev:browser` script to `src/crosshook-native/package.json` (alias for `vite`, semantic clarity)
- **1.3** Force `server.host = '127.0.0.1'` and `server.strictPort = true` in `vite.config.ts`
- **1.4** Create `src/crosshook-native/src/lib/` with:
  - `runtime.ts` — `isTauri()` probe
  - `ipc.ts` — `callCommand<T>` adapter with dynamic mock import + distinctive unhandled-command error
  - `events.ts` — `subscribeEvent` adapter + in-process event bus + `emitMockEvent`
  - `DevModeBanner.tsx` — fixed top stripe rendered when `!isTauri()`
- **1.5** Create `src/crosshook-native/src/lib/plugin-stubs/`:
  - `dialog.ts` — re-exports real plugin in Tauri mode; `console.warn` + visible toast in browser
  - `shell.ts`, `fs.ts` — same pattern
  - `convertFileSrc.ts` — real in Tauri mode, placeholder data URL in browser
- **1.6** Create `src/crosshook-native/src/lib/mocks/`:
  - `index.ts` — `registerMocks()` returns `Map<string, Handler>` merged from all area modules
  - `store.ts` — in-memory `Map` for optional round-trip state
  - `eventBus.ts` — wires handler `emitMockEvent` calls into `lib/events.ts`
  - `README.md` — how to add a handler (with one worked example)
  - `settings.ts` — handlers for `settings_load`, `recent_files_load`, `recent_files_save`, `default_steam_client_install_path` (uses `DEFAULT_APP_SETTINGS`)
  - `profiles.ts` — handlers for `profile_list`, `profile_list_favorites`, `profile_list_summaries` (uses `createDefaultProfile()`)
  - `system.ts` — handlers for `check_readiness`, `get_optimization_catalog`, `batch_validate_profiles` (the remaining boot-blocking commands)
- **1.7** Export `ProfileSummary` from `types/library.ts` (practices-researcher's note)
- **1.8** Mechanical migration:
  - Replace `import { invoke } from '@tauri-apps/api/core'` with `import { callCommand } from '@/lib/ipc'`
  - Replace all `invoke(` with `callCommand(` (84 sites in 42 files)
  - Replace `import { listen } from '@tauri-apps/api/event'` with `import { subscribeEvent } from '@/lib/events'`
  - Replace all `listen(` with `subscribeEvent(` (16 sites in 13 files)
  - Migrate plugin imports to use the stubs
- **1.9** Render `<DevModeBanner />` from `App.tsx` above `<ProfileProvider>` (following the existing `RouteBanner`/`OfflineStatusBadge` pattern); self-gates on `!isTauri()`
- **1.10** Add CI smoke test that greps `dist/` for `lib/mocks`, `registerMocks`, `MOCK MODE`, and `[dev-mock] callCommand` — fail the build job on any hit
- **1.11** Update `AGENTS.md` "Commands" section to mention the new `--browser` flag
- **1.12** Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` (no Rust changes expected, but the migration touches a lot of TS)

### Phase 2 — On-demand handler coverage

**Goal:** As contributors iterate on a specific area, they claim a sub-phase and add mocks. No front-loading. Each sub-phase is its own PR.

- **2.1** **Launch** handlers + events (`launch_game`, `launch_trainer`, `preview_launch`, `validate_launch`, `check_game_running`, `verify_trainer_hash`, `check_gamescope_session`, `run_executable` family; events: `launch-log`, `launch-diagnostic`, `launch-complete`, `profiles-changed`) — add to `lib/mocks/launch.ts`
- **2.2** **Health Dashboard** handlers + events (`get_profile_health`, `get_cached_health_snapshots`, `check_offline_readiness` family, `protondb_*`; events: `profile-health-batch-complete`, `version-scan-complete`) — add to `lib/mocks/health.ts`
- **2.3** **Onboarding events** (`onboarding-check`, `auto-load-profile`, `dismiss_onboarding`, `check_version_status`) — add to `lib/mocks/system.ts`
- **2.4** **Install flow** (`install_game`, `validate_install_request`, `install_default_prefix_path`) — `lib/mocks/system.ts`
- **2.5** **Update flow** (`update_game`, `validate_update_request`, `cancel_update`; events: `update-complete`, `update-log`) — `lib/mocks/system.ts`
- **2.6** **Proton stack** (`list_proton_installs`, `check_proton_migrations`, `apply_proton_migration`, `apply_batch_migration`, `protonup_*`) — `lib/mocks/system.ts`
- **2.7** **Community** (`community_list_profiles`, `community_list_indexed_profiles`, `community_sync`, `community_add_tap`, `community_prepare_import`, `community_export_profile`, `discovery_search_trainers`) — `lib/mocks/community.ts`
- **2.8** **Launcher export** (`list_launchers`, `check_launcher_exists`, `validate_launcher_export`, `export_launchers`, `preview_launcher_*`, `delete_launcher*`) — `lib/mocks/system.ts`
- **2.9** **Remaining `system.ts` additions** — prefix storage, diagnostics, catalog, art, steam — added as encountered
- **2.10** **Profile mutation handlers** (`profile_save`, `profile_duplicate`, `profile_rename`, `profile_export_toml`, optimization preset commands) — round-trip via `lib/mocks/store.ts` if needed
- **2.11** **Wire `profiles-changed` event broadcast** on profile-mutating handlers so UI refreshes naturally
- **2.12** **Library handlers** (`fetch_game_cover_art`, `import_custom_art`, `fetch_game_metadata`, `auto_populate_steam`, `build_steam_launch_options_command`) — `lib/mocks/library.ts`

### Phase 3 — Polish

- **3.1** Fixture variants per domain (empty, loading, error, populated) selectable via `?scenario=` URL param
- **3.2** Optional: handler-coverage check script (`pnpm dev:browser:check`) that scans `commands/*.rs` and lists unmocked commands
- **3.3** Document the dev-browser flow in the project README
- **3.4** Optional: Playwright smoke test that boots the dev server and screenshot-checks every route (incl. boot-sequence rendering sanity check)
- **3.5** Track `refactor:` follow-up issue for the 13 components calling `invoke` directly

### Persistence Boundary Statement

Per `AGENTS.md` requirements for any feature touching data:

- **TOML settings**: None added.
- **SQLite metadata**: None added.
- **Runtime-only state**: All mock data is **runtime-only**, in-process, in-browser memory (`lib/mocks/store.ts` if session round-trip is needed). Resets on page reload. No persistence layer involved.
- **Migration / backward compat**: N/A. The `lib/` adapter is always present (it just routes to real Tauri in production); the `lib/mocks/` subtree is dev-only, gated on `!isTauri()`, and never reaches production.
- **Offline behavior**: The mock layer is _always_ offline; it has no network calls. Browser dev mode works with the laptop on a plane.
- **Degraded fallback**: If the mock layer fails to dynamic-import, `callCommand` rejects with a clear error message. If a handler is missing for a command, the same fail-loud behavior surfaces. Better than silently rendering with no data.
- **User visibility / editability**: Contributors edit handler files in `src/crosshook-native/src/lib/mocks/`. Variants switchable via URL query (Phase 3). End users never see browser mode.

---

## Key Decisions Needed

1. **Flag name: `--browser` vs `--web` vs `--dev`?** Recommend **`--browser`** as primary (most explicit about what it does), with `--web` as alias. `--dev` is too generic.
2. **Vite host binding**: confirm we force `127.0.0.1` and reject `--host` overrides? Strongly recommend yes.
3. **Banner mount point**: from `App.tsx` above `<ProfileProvider>`, following the existing `RouteBanner`/`OfflineStatusBadge` pattern. Confirmed by practices-researcher.
4. **Banner copy**: recommend "BROWSER MODE — Tauri IPC is mocked. Re-verify UI in `./scripts/dev-native.sh` before merging." Who owns the visual?
5. **Single-PR Phase 1 vs split**: ship foundation + adapter migration + boot-blocking handlers in one PR, or split? Recommend **one PR**: the migration is mechanical and the foundation is meaningless without something rendering. Size estimate: ~50 files of mechanical changes + new `lib/` directory. Coordinate with team-lead before opening.
6. **Plugin stub semantics**: confirm they fail loud (console.warn + visible toast), never silent no-op. Strongly recommend yes.
7. **Mock layer console logging**: recommend yes by default in browser mode (`console.debug('[dev-mock] callCommand', name, args)`) — invaluable for contributors.
8. **CI gate sentinel strings**: `lib/mocks`, `registerMocks`, `MOCK MODE`, `[dev-mock] callCommand`. All four? Recommend yes for defense-in-depth.
9. **`@/lib/...` path alias**: introduce a Vite path alias for `lib/` as part of Phase 1, or relative imports only? Recommend the alias — makes imports cleaner and eases future refactors.

---

## Open Questions

1. **What does the security-researcher's full report say?** Their CRITICAL findings should slot into the [Risk Assessment](#risk-assessment) above. The synthesis was written before their report arrived; revisit when available.
2. **Does tech-designer's final technical design align with approach (a)?** I've voted Option A in their parallel thread; awaiting their finalized doc.
3. **Does the api-researcher's library scan turn up anything useful?** None of the alternatives considered are clear wins, but they may have surfaced something specific to React/Vite.
4. **Does the UX-researcher's banner guidance accept the inline-styled `<div>` recommendation?** If they propose a richer banner we may need to accept some scope creep.
5. **Are there Tauri IPC commands that throw on the Rust side under conditions the mock can't replicate?** (e.g. file-system race conditions, gamescope detection.) Document those as "behavior differs in browser mode" in `lib/mocks/README.md`.
6. **Will a future contract-test follow-up actually happen?** Cheaper to do now if it's coming; over-engineering if it isn't. Defer to team-lead.
7. **Do we need a Playwright dev dependency for the CI smoke test, or is grep-on-dist sufficient for Phase 1?** Grep is the cheaper Phase 1 control.
8. **Tauri events with payload schemas**: the event bus shim uses `subscribeEvent<T>` (typed), matching existing `listen<T>` call sites. Confirmed sufficient.

---

## Sources & References

- Existing CrossHook IPC surface: 88 distinct commands / ~84 call sites / 138 `invoke[<(]` occurrences across 42 files (per practices-researcher and verified via `grep invoke[<(]` in `src/crosshook-native/src/`)
- Existing Tauri event surface: 16 `listen()` call sites across 13 files
- Existing Tauri plugin usage: `@tauri-apps/plugin-dialog`, `@tauri-apps/plugin-shell`, `@tauri-apps/plugin-fs` across 6 files
- `convertFileSrc` usage: `useGameCoverArt.ts` and `MediaSection.tsx`
- Existing dev script: `scripts/dev-native.sh`
- Existing package config: `src/crosshook-native/package.json` (already depends on `@tauri-apps/api ^2.0.0`)
- Repo conventions: `AGENTS.md` and `CLAUDE.md` (Tauri v2, snake_case commands, Serde boundary, native-Linux platform)
- Tauri v2 mock API: `@tauri-apps/api/mocks` — `mockIPC`, `clearMocks`, `mockWindows`. **Works in plain browsers** (verified twice against upstream `tauri-apps/tauri:packages/api/src/mocks.ts`, function `mockInternals()` uses `??` to create `window.__TAURI_INTERNALS__` if absent, then replaces `__TAURI_INTERNALS__.invoke`). Rejected for this use case on tree-shake-provability, coverage-surface, and error-distinctiveness grounds, _not_ for browser compatibility.
- Practices-researcher's findings: `docs/plans/dev-web-frontend/research-practices.md` (build-vs-depend verdict, ready-made fixture inventory, boot-blocking command list)
- Tech-designer's technical design: `docs/plans/dev-web-frontend/` (Option A vote, tree-shake-provability rationale)
