# Business Requirements: dev-web-frontend Mode

## Executive Summary

CrossHook's frontend (`src/crosshook-native/src/`) currently requires a full Tauri build to run, because every hook and context provider calls `invoke()` or `listen()` directly from `@tauri-apps/api`. This feature introduces a `--dev` flag to `./scripts/dev-native.sh` that starts only the Vite dev server (port 5173) with mock data, enabling UI/UX iteration in a regular browser without Rust toolchain or a running Tauri process. No production code path changes. All mock data is ephemeral runtime state only — nothing is persisted.

---

## User Stories

### US-1: Developer iterates on UI without compiling Rust

**As** a frontend developer or designer contributing to CrossHook,  
**I want** to run `./scripts/dev-native.sh --dev` and open `http://localhost:5173` in my browser,  
**So that** I can inspect elements, use DevTools, iterate on CSS, and verify component layouts without waiting for a full Tauri build.

**Acceptance criteria:**

- `./scripts/dev-native.sh --dev` starts only Vite, does not invoke `cargo` or `tauri`.
- `http://localhost:5173` renders the full CrossHook app shell with mock data in all major views.
- HMR (hot module replacement) reloads changed components in the browser within Vite's normal latency.
- All 9 routes (`library`, `profiles`, `launch`, `install`, `community`, `discover`, `compatibility`, `settings`, `health`) render without crashing.

### US-2: Contributor without Rust toolchain evaluates UI

**As** a new contributor or designer who has Node.js but not Rust installed,  
**I want** to preview the CrossHook interface in a browser,  
**So that** I can make CSS or component contributions without setting up a native build environment.

**Acceptance criteria:**

- Running `npm run dev` inside `src/crosshook-native/` (or `./scripts/dev-native.sh --dev`) completes without any Rust/Cargo dependency.
- The app renders representative mock data for the visible screens.

### US-3: Mock mode is visually distinguishable from production

**As** a developer using the web-dev mode,  
**I want** a persistent fixed-position chip that labels the active fixture state,  
**So that** I cannot accidentally confuse a mock session with a running Tauri app, regardless of sidebar state or active route.

**Acceptance criteria:**

- A `position: fixed; bottom: 12px; right: 12px; z-index: 9999` chip displays `DEV · {fixture}` at all times.
- Background: `--crosshook-autosave-warning-bg` (`rgba(245,197,66,0.14)`), border: `--crosshook-autosave-warning-border`, text: `--crosshook-color-warning` uppercase with `letter-spacing: 0.08em`. Approximately 120×24 px at `0.7rem`.
- The chip carries `role="status"` and `aria-label="Developer mode active. Fixture: {fixture}"`.
- The chip meets WCAG AA contrast: 8.5:1 ratio confirmed against the background token.
- The chip cannot be dismissed — it has no close button or interactive affordance.
- The chip is visible on all 9 routes, regardless of sidebar collapsed/expanded state.
- The chip does not appear in production (Tauri) builds — conditioned on `import.meta.env.DEV && import.meta.env.VITE_WEB_DEV === 'true'`.

### US-4: Developer switches fixture state without rebuilding

**As** a developer iterating on UI states,  
**I want** to switch between named fixture states by changing a URL query parameter,  
**So that** I can test empty lists, error banners, and loading spinners without touching code.

**Acceptance criteria:**

- `http://localhost:5173` (no query param) loads the `populated` fixture state by default.
- `http://localhost:5173?fixture=empty` renders all list views empty (zero profiles, zero community entries, etc.).
- `http://localhost:5173?fixture=error` exercises all error UI paths (profile load error, settings error, health check error, etc.).
- `http://localhost:5173?fixture=loading` shows all async views in their loading/spinner state.
- Switching states requires only a URL change — no Vite restart, no code change, no rebuild.
- Unknown `?fixture=` values fall back to `populated`.

### US-5: Default fixture shows a realistic, populated UI

**As** a developer or designer opening the dev server for the first time,  
**I want** `npm run dev` to immediately show a realistic, data-rich UI,  
**So that** I can evaluate layout, spacing, and typography on real-looking content without configuring anything.

**Acceptance criteria:**

- The default `populated` fixture includes at least 3 sample profiles with realistic game names, exe paths, and cover art placeholders.
- All sidebar routes navigate to a page with primary content visible (no empty states or loading spinners).
- The Library page shows a populated grid or list.
- The Health page shows at least one profile health row.

### US-6: Developer inspects error UI in context of a fully populated interface

**As** a developer,  
**I want** to enable write/action errors independently of the data fixture,  
**So that** I can see error banners and toasts overlaid on a realistic populated UI, rather than on an empty shell.

**Acceptance criteria:**

- Adding `?errors=true` to any URL causes all mutating/action commands (`profile_save`, `profile_delete`, `launch_game`, `install_game`, etc.) to reject with a realistic error string.
- Read commands (`profile_list`, `settings_load`, `profile_load`, etc.) continue to return the current fixture's data unaffected.
- The chip label reflects both states, e.g. `DEV · populated · errors`.
- `?errors=true` is orthogonal to `?fixture=<name>` — both can be combined.

### US-7: Developer reaches the onboarding wizard in browser mode

**As** a developer working on the onboarding flow,  
**I want** a URL toggle that surfaces the onboarding wizard,  
**So that** I can iterate on wizard UI that is otherwise unreachable in browser mode (since the `onboarding-check` Tauri event never fires).

**Acceptance criteria:**

- Adding `?onboarding=show` to the URL causes the onboarding wizard to open on app mount, equivalent to the backend emitting `onboarding-check` with `{ show: true, has_profiles: false }`.
- Without `?onboarding=show`, the wizard does not open (default behavior).
- The wizard renders all its stages in browser mode without errors.

### US-8: Developer simulates slow network/IO for Steam Deck parity

**As** a developer checking loading states and skeleton layouts,  
**I want** to add an artificial delay to all async mock responses,  
**So that** I can inspect loading indicators that would otherwise flash by instantly.

**Acceptance criteria:**

- Adding `?delay=<ms>` (e.g. `?delay=800`) wraps every mock `callCommand` response in a `setTimeout` of the given duration before resolving.
- The documented example (`?delay=800`) simulates a realistic slow Steam Deck IO latency.
- `?delay` is orthogonal to `?fixture=` and `?errors=true` — all three can be combined.
- Without `?delay`, responses resolve immediately (no added latency).

---

## Business Rules

### BR-1: No mock code in production builds

Mock fixtures, the IPC adapter shim, and the dev-mode chip **must not** be included in production bundles. Enforcement mechanism: Vite `import.meta.env.DEV` (already declared in `src/crosshook-native/src/vite-env.d.ts`) is `true` only during `vite dev`. Mock modules must be either:

- Tree-shaken via conditional `if (import.meta.env.DEV)` guards, OR
- Conditionally imported via a dedicated Vite alias (`@tauri-apps/api/core` → mock) that applies only when `VITE_WEB_DEV=true`.

### BR-2: Mock mode must not require Rust toolchain

The `--dev` script path must not call `cargo`, `tauri`, or any native binary. Only `npm` / `node` are allowed. This allows designers and frontend contributors to use the mode without a Rust installation.

### BR-3: Same React component tree — no divergent code paths

The identical React components, hooks, and context providers used in production must be used in mock mode. The IPC boundary (Tauri `invoke`/`listen`) is the only swapped layer. No duplicate page or route components are acceptable.

### BR-4: Mock data must cover visible UI surfaces

Each route that calls `invoke()` must receive realistic fixture data so that the UI renders its primary content areas (lists, cards, forms) rather than loading spinners or error states. Minimal acceptable mock coverage = the 9 routes enumerated in US-1.

### BR-5: Mock write-backs are no-ops (no state mutations that persist)

Mock implementations of mutating IPC calls (`profile_save`, `settings_save`, etc.) must complete without error but may discard data. Mock state updates that affect UI within a session are a UX nicety, not a requirement, and must be documented if implemented.

### BR-6: Tauri events (`listen`) that do not fire must not break the UI

Many hooks register `listen()` handlers for events like `profiles-changed`, `launch-complete`, `onboarding-check` (see §Existing Codebase Analysis). The mock adapter must provide a no-op `listen` that returns a no-op unsubscribe function; components must not crash when these events never fire.

### BR-7: Default fixture state is `populated`

When no `?fixture=` query parameter is present, the app must load the `populated` fixture. This is the primary use case — design iteration on a real-looking UI. An empty or loading default shell is unacceptable.

### BR-8: Fixture state is controlled by URL query string; no rebuild required

The four fixture states (`populated`, `empty`, `error`, `loading`) must be selectable at runtime via `?fixture=<name>`. The fixture resolver reads `window.location.search` on app init. Switching states requires only a URL navigation — no Vite restart, no env change, no code modification.

### BR-9: Dev-mode chip must be `position: fixed` — not inside the sidebar

The chip must not be placed in the sidebar brand area or status group. The sidebar applies `display: none` to both `crosshook-sidebar__brand` and `crosshook-sidebar__status-group` when `data-collapsed="true"` (`sidebar.css` lines 203–237), which would hide the chip whenever the sidebar is collapsed — violating the "visible on all 9 routes" requirement of US-3. `position: fixed; bottom: 12px; right: 12px` is outside the sidebar's flex/grid context entirely and is unaffected by sidebar state, route transitions, modal layers, or scroll.

### BR-10: `?errors=true` is an orthogonal toggle; read commands must not be affected

When `?errors=true` is present, only mutating/action commands reject. Read commands (`*_load`, `*_list`, `*_get`, `profile_load`, etc.) must continue to return their fixture data. This rule prevents the app shell from crashing when errors are enabled while in `populated` fixture state. The `callCommand` registry must classify commands as read or write to enforce this boundary.

### BR-11: Onboarding wizard requires explicit `?onboarding=show` opt-in

The `onboarding-check` Tauri event is never emitted in browser mode, so the `showOnboarding` state in `App.tsx:64-72` will never be set to `true` by normal mock event flow. The `?onboarding=show` URL param must trigger the wizard on app mount as a direct state injection, bypassing the event mechanism. This is the only path to reach the onboarding wizard UI in browser dev mode.

### BR-12: `?delay=<ms>` wraps all mock responses; zero by default

When `?delay=N` is present, every `callCommand` response is deferred by `N` milliseconds using `setTimeout`. Without `?delay`, responses resolve in the same microtask tick. The delay applies uniformly to all commands regardless of fixture state or `?errors=true` state.

---

## Architectural Decisions

The following decisions are settled based on cross-team analysis and apply to implementation planning.

### AD-1: `callCommand` wrapper for `@tauri-apps/api/core`; Vite alias for the three plugin modules

`invoke` from `@tauri-apps/api/core` must be replaced with a `callCommand` wrapper at every callsite (~84 files + 13 components). This is a mechanical codemod (`invoke(` → `callCommand(`), but makes every IPC callsite explicit and enables a typed command registry. The alias approach would hide the same registry complexity inside `mock-invoke.ts` without saving any implementation work.

The three peripheral modules use Vite `resolve.alias` (clean swap, zero callsite changes):

- `@tauri-apps/api/event` → `src/lib/mocks/tauri-event.ts` (exports `listen: async () => () => {}`)
- `@tauri-apps/plugin-dialog` → `src/lib/mocks/tauri-dialog.ts` (exports `open`, `save` as no-ops returning `null`)
- `@tauri-apps/plugin-shell` → `src/lib/mocks/tauri-shell.ts` (exports `open` as no-op)

`convertFileSrc` from `@tauri-apps/api/core` is handled as a passthrough in the `callCommand` module: `(path: string) => path`. This allows fixture strings to render directly as image `src` values.

### AD-2: Mock modules live in `src/lib/mocks/`

Not `src/platform/web/fixtures/`. The `src/crosshook-native/src/platform/web/fixtures/` directory that exists in the repo is empty and does not yet establish a convention — `lib/mocks/` is the correct location as it sits alongside `lib/ipc.ts` and `lib/runtime.ts`, communicates library-level scope, and is not confused with page-level fixture data.

### AD-3: `window.__TAURI_INTERNALS__` is the designed API for detecting Tauri runtime

Not `process.env.TAURI_DEV_HOST` (undocumented side-effect of `tauri dev`). The runtime mode check must use `typeof window.__TAURI_INTERNALS__ !== 'undefined'` as the canonical signal that the app is running inside a Tauri webview. `TAURI_DEV_HOST` being undefined is fragile and not a designed API.

### AD-4: `callCommand` adapter exports the fixture-state dispatch table

The `callCommand` function reads the active fixture state (resolved once at module init from `window.location.search`) and routes each command name to the appropriate handler. The four fixture states (`populated`, `empty`, `error`, `loading`) are implemented as separate handler maps, not conditional branches inside individual command handlers.

---

## Workflows

### Primary Developer Workflow

1. Developer runs `./scripts/dev-native.sh --dev` (or directly `cd src/crosshook-native && npm run dev`).
2. The script detects `--dev` flag and skips Tauri; it runs only `npm exec vite` (equivalent to `npm run dev`).
3. Vite starts on port 5173. The presence of `VITE_WEB_DEV=true` (or a custom Vite mode `web-dev`) activates the mock IPC adapter.
4. Browser opens `http://localhost:5173` — the `populated` fixture loads by default.
5. The mock adapter shim intercepts all `@tauri-apps/api` imports and returns fixture data.
6. The app renders with the `DEV · populated` chip visible.
7. Developer edits CSS or React components. HMR updates the browser immediately.
8. Developer navigates between routes; each route renders from fixture data.
9. Developer closes the browser or stops the Vite process with `Ctrl+C`.

### Fixture State Switching Workflow

1. Developer is in the browser at `http://localhost:5173` (default `populated` state).
2. Developer appends `?fixture=empty` to the URL and presses Enter.
3. The app reloads; all list views now return empty arrays from the mock adapter.
4. The chip updates to `DEV · empty`.
5. Developer inspects empty-state UI (zero profiles, empty community list, etc.).
6. Developer changes to `?fixture=error` to exercise error banners and recovery UI.
7. Developer changes to `?fixture=loading` to inspect skeleton/spinner layouts.
8. Developer returns to `http://localhost:5173` (or `?fixture=populated`) for default design iteration.

### Tauri Production Workflow (unchanged)

`./scripts/dev-native.sh` (without `--dev`) continues to behave exactly as today: it runs `npm exec tauri dev` which starts both the Vite dev server and the Tauri native process with real IPC.

---

## Domain Model

| Concept            | Definition                                                                                                                                                                                                                                                        |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Runtime mode**   | One of `tauri` (production IPC, running Tauri process) or `web-mock` (browser-only, no Tauri process). Detected at app startup via `import.meta.env.DEV` and/or a custom env variable `VITE_WEB_DEV`.                                                             |
| **IPC adapter**    | A `callCommand(cmd, args?)` wrapper that replaces `invoke()` at every callsite. In `web-mock` mode it dispatches to a fixture-state-aware handler registry; in `tauri` mode it delegates to the real `invoke`.                                                    |
| **Mock fixture**   | A static TypeScript object or factory function that returns data shaped like the real IPC response type. Lives in `src/crosshook-native/src/lib/mocks/` alongside the `callCommand` registry.                                                                     |
| **Fixture state**  | One of four named data scenarios: `populated` (default — realistic data), `empty` (empty lists, zero records), `error` (all fallible IPC calls reject), `loading` (all async calls pend indefinitely). Selected at runtime via `?fixture=<name>` URL query param. |
| **Dev-mode chip**  | A persistent corner UI element rendered only in `web-mock` mode displaying `DEV · {fixture}`. Uses `--crosshook-autosave-warning-*` CSS token colors (amber). Must meet WCAG 8.5:1 contrast. Never rendered in production.                                        |
| **Mock event bus** | A minimal replacement for `@tauri-apps/api/event`'s `listen()` that accepts a handler but never calls it; returns `Promise.resolve(() => {})` to satisfy the cleanup pattern used across all hooks.                                                               |

---

## Existing Codebase Integration

### Entry Point and App Shell

`src/crosshook-native/src/main.tsx` mounts `<App />` unconditionally — no env checks (`src/crosshook-native/src/main.tsx:13`). The App tree is:

```
App (App.tsx:141)
  ProfileProvider (context/ProfileContext.tsx)
    ProfileHealthProvider (context/ProfileHealthContext.tsx)
      AppShell
        PreferencesProvider (context/PreferencesContext.tsx)
          LaunchStateProvider (context/LaunchStateContext.tsx)
            Tabs.Root (9 routes via ContentArea)
```

Every context provider calls `invoke()` on mount. Mock mode must ensure these calls succeed before the UI can render.

### Routes and Pages

All 9 routes are defined in `src/crosshook-native/src/components/layout/Sidebar.tsx:16`:

```typescript
export type AppRoute =
  | 'library'
  | 'profiles'
  | 'launch'
  | 'install'
  | 'community'
  | 'discover'
  | 'compatibility'
  | 'settings'
  | 'health';
```

Routed through `ContentArea.tsx` (`src/crosshook-native/src/components/layout/ContentArea.tsx:20`).

Sidebar navigation groups (`src/crosshook-native/src/components/layout/Sidebar.tsx:36`):

- **Game**: Library, Profiles, Launch
- **Setup**: Install
- **Dashboards**: Health
- **Community**: Community, Discover, Compatibility

### IPC Call Inventory (exhaustive)

All 80+ IPC commands discovered via grep across hooks, context providers, and page components:

**Context providers (called on mount — critical path):**

| Command                                  | File                                | Return type              |
| ---------------------------------------- | ----------------------------------- | ------------------------ |
| `settings_load`                          | `context/PreferencesContext.tsx:44` | `AppSettingsData`        |
| `recent_files_load`                      | `context/PreferencesContext.tsx:45` | `RecentFilesData`        |
| `default_steam_client_install_path`      | `context/PreferencesContext.tsx:46` | `string`                 |
| `profile_list`                           | `hooks/useProfile.ts`               | `string[]`               |
| `profile_list_favorites`                 | `hooks/useProfile.ts`               | `string[]`               |
| `profile_load`                           | `hooks/useProfile.ts`               | `SerializedGameProfile`  |
| `get_cached_health_snapshots`            | `hooks/useProfileHealth.ts`         | `CachedHealthSnapshot[]` |
| `batch_validate_profiles`                | `hooks/useProfileHealth.ts`         | `EnrichedHealthSummary`  |
| `get_cached_offline_readiness_snapshots` | `hooks/useOfflineReadiness.ts`      | object                   |

**Tauri events listened on mount (must not crash if never fired):**

| Event                             | File                                     | Purpose                        |
| --------------------------------- | ---------------------------------------- | ------------------------------ |
| `auto-load-profile`               | `context/ProfileContext.tsx:36`          | Auto-select profile at startup |
| `onboarding-check`                | `App.tsx:67`                             | Trigger onboarding wizard      |
| `profiles-changed`                | `hooks/useProfile.ts:1307`               | Refresh profile list           |
| `profiles-changed`                | `hooks/useCommunityProfiles.ts:425`      | Refresh community index        |
| `profile-health-batch-complete`   | `hooks/useProfileHealth.ts:152`          | Update health summaries        |
| `launch-complete`                 | `hooks/useProfileHealth.ts:198`          | Post-launch health refresh     |
| `version-scan-complete`           | `hooks/useProfileHealth.ts:202`          | Version status refresh         |
| `offline-readiness-scan-complete` | `hooks/useOfflineReadiness.ts:121`       | Refresh offline state          |
| `launch-log`                      | `components/layout/ConsoleDrawer.tsx:74` | Stream log lines               |
| `update-log`                      | `components/layout/ConsoleDrawer.tsx:75` | Stream update log lines        |
| `launch-diagnostic`               | `hooks/useLaunchState.ts:231`            | Receive diagnostic report      |
| `launch-complete`                 | `hooks/useLaunchState.ts:247`            | Signal launch done             |
| `update-complete`                 | `hooks/useUpdateGame.ts:232`             | Signal update done             |
| `run-executable-complete`         | `hooks/useRunExecutable.ts:186`          | Signal executable done         |

**Per-route IPC (called on navigation or user interaction — secondary priority):**

| Route / Component | Key commands                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Library           | `profile_list_summaries`, `profile_list_favorites`, `check_offline_readiness`                                                                                                                                                                                                                                                                                                                                                                                                   |
| Profiles          | `profile_load`, `profile_save`, `profile_delete`, `profile_duplicate`, `profile_rename`, `profile_config_history`, `profile_config_diff`, `profile_config_rollback`, `profile_set_favorite`, `profile_list_bundled_optimization_presets`, `profile_save_gamescope_config`, `profile_save_trainer_gamescope_config`, `profile_save_mangohud_config`, `profile_save_launch_optimizations`, `profile_apply_bundled_optimization_preset`, `profile_save_manual_optimization_preset` |
| Launch            | `validate_launch`, `launch_game`, `launch_trainer`, `check_version_status`, `check_offline_readiness`, `preview_launch`, `get_optimization_catalog`, `get_mangohud_presets`, `get_trainer_type_catalog`                                                                                                                                                                                                                                                                         |
| Install           | `validate_install_request`, `install_game`, `validate_run_executable_request`, `run_executable`, `cancel_run_executable`, `detect_protontricks_binary`, `check_gamescope_session`, `check_readiness`                                                                                                                                                                                                                                                                            |
| Community         | `community_list_indexed_profiles`, `community_list_profiles`, `community_sync`, `community_add_tap`, `community_prepare_import`, `community_import_profile`, `community_export_profile`                                                                                                                                                                                                                                                                                         |
| Discover          | `discovery_search_trainers`, `get_optimization_catalog`                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Compatibility     | `protondb_lookup`, `protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion`, `protonup_get_suggestion`, `protonup_list_available_versions`, `protonup_install_version`                                                                                                                                                                                                                                                                           |
| Settings          | `settings_load`, `settings_save`, `settings_save_steamgriddb_key`, `default_steam_client_install_path`, `detect_protontricks_binary`, `build_steam_launch_options_command`, `export_diagnostics`                                                                                                                                                                                                                                                                                |
| Health            | `get_profile_health`, `get_cached_health_snapshots`, `check_version_status`, `acknowledge_version_change`, `profile_mark_known_good`, `batch_offline_readiness`                                                                                                                                                                                                                                                                                                                 |

**Tauri plugins (beyond `invoke`/`listen`):**

| Plugin                      | Usage                                                                                                    |
| --------------------------- | -------------------------------------------------------------------------------------------------------- |
| `@tauri-apps/plugin-dialog` | `open()` / `save()` — wrapped in `src/crosshook-native/src/utils/dialog.ts`; used in file picker fields  |
| `@tauri-apps/plugin-shell`  | `open()` — opens URLs/files in OS default handler; used in Community, Discover, Settings, ProtonDB pages |
| `@tauri-apps/api/core`      | `convertFileSrc()` — converts filesystem paths to tauri:// asset URLs for cover art                      |

### Existing Vite/Env Infrastructure

- `vite.config.ts` already sets `envPrefix: ['VITE_', 'TAURI_ENV_*']` (`src/crosshook-native/vite.config.ts:25`).
- `vite-env.d.ts` already declares `ImportMetaEnv` with `DEV`, `PROD`, `MODE`, `BASE_URL` (`src/crosshook-native/src/vite-env.d.ts`).
- Only one existing `import.meta.env` usage exists: `useGamepadNav.ts:180` checks `VITE_STEAM_DECK`.
- No existing web-mock or dev-only code paths are present.
- The `src/crosshook-native/src/platform/web/fixtures/` directory exists but is empty and does not yet establish a convention. Per AD-2, mock modules belong in `src/crosshook-native/src/lib/mocks/`.

### Component Architecture Notes

- **Scroll enhancement**: `useScrollEnhance` (`hooks/useScrollEnhance.ts`) targets specific selectors. New overflow containers must be registered there (AGENTS.md requirement).
- **Route layout classes**: established CSS contract (`crosshook-page-scroll-shell--fill`, `crosshook-route-stack`, etc.) documented in AGENTS.md. Mock mode renders the same layout.
- **`listen()` cleanup pattern**: all event subscriptions are cleaned up via returned unlisten functions in `useEffect` teardowns. The mock `listen` must return a `Promise<() => void>` to match the real API shape.
- **Scroll feel in browser vs Tauri**: WebKitGTK scroll behavior differs from Chrome/Firefox. This difference is expected and documented — scroll-feel parity is not an acceptance criterion for browser dev mode.

---

## Storage Boundary

This feature introduces **no persistent data**. All mock state is:

| Data                        | Classification               | Reason                                                                                           |
| --------------------------- | ---------------------------- | ------------------------------------------------------------------------------------------------ |
| Mock profile list, fixtures | **Runtime-only (ephemeral)** | Defined as static TypeScript constants; discarded when the browser tab or Vite process is closed |
| Mock settings values        | **Runtime-only (ephemeral)** | In-memory defaults from `DEFAULT_APP_SETTINGS`; never written to `settings.toml`                 |
| Dev-mode chip visibility    | **Runtime-only (ephemeral)** | Determined by `import.meta.env.DEV`; no user preference stored                                   |
| Active fixture state        | **Runtime-only (ephemeral)** | Read from `window.location.search` on init; not stored in localStorage or any persistent layer   |
| Mock event bus state        | **Runtime-only (ephemeral)** | Held in React state if simulated events are implemented; no persistence                          |

No new TOML settings fields are required. No SQLite migrations are required. No new `external_cache_entries` or `trainer_hash_cache` rows are written.

---

## Success Criteria

| ID    | Criterion                                                                  | Verifiable by                                                                       |
| ----- | -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| SC-1  | `./scripts/dev-native.sh --dev` starts Vite without invoking Rust/Cargo    | Observing process tree: no `cargo` or `tauri` binary spawned                        |
| SC-2  | `http://localhost:5173` renders all 9 routes without JS errors in DevTools | Manual navigation through all routes; console clean                                 |
| SC-3  | `DEV · populated` chip is visible on all routes in default mode            | Visual inspection                                                                   |
| SC-4  | Production build (`npm run build`) does not include mock modules           | `vite build` produces a bundle with no references to fixture files                  |
| SC-5  | HMR works: editing a CSS variable in `variables.css` reflects in <2s       | Timed manual test                                                                   |
| SC-6  | `listen()` calls do not throw or crash when events never fire              | Load app, wait 5s, no console errors from listener teardown                         |
| SC-7  | File picker (`chooseFile`, `chooseDirectory`) does not crash in mock mode  | Click a "Browse" button; receives a `null` return without throwing                  |
| SC-8  | `convertFileSrc` called for cover art does not crash                       | Cover art cells in Library render a placeholder or empty without error              |
| SC-9  | `?fixture=empty` shows all list views empty with no JS errors              | Navigate to `?fixture=empty`; Library grid shows empty state; console clean         |
| SC-10 | `?fixture=error` exercises error UI paths without uncaught exceptions      | Navigate to `?fixture=error`; error banners visible; no uncaught promise rejections |
| SC-11 | `?fixture=loading` shows spinner/skeleton states                           | Navigate to `?fixture=loading`; async content shows loading indicators              |
| SC-12 | Dev chip label updates to match active fixture state                       | Switch between all 4 states; chip label matches each                                |

---

## Settled Decisions (formerly Open Questions)

The following were open questions resolved by cross-team analysis. See §Architectural Decisions for the normative record.

| #   | Question                                  | Decision                                                                                                            |
| --- | ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Q1  | Vite mode vs env variable                 | Use a named Vite mode (`--mode web-dev`) for cleaner `define` aliasing; `VITE_WEB_DEV` is set as part of that mode. |
| Q2  | Alias strategy for `@tauri-apps/api/core` | **Resolved**: `callCommand` wrapper at callsites; Vite alias only for the three plugin modules. See AD-1.           |
| Q4  | `convertFileSrc` in web mode              | **Resolved**: passthrough `(path) => path` in `callCommand` module; fixture strings render as image `src` directly. |
| Q5  | `@tauri-apps/plugin-dialog` mock          | **Resolved**: Vite alias to `src/lib/mocks/tauri-dialog.ts`; `open`/`save` return `null`. See AD-1.                 |

## Open Questions

1. **Vite mode vs env variable**: Use `--mode web-dev` (named Vite mode). Requires a `vite.config.ts` mode branch or `.env.web-dev` file to set `VITE_WEB_DEV=true`. The named mode also enables `vite build --mode web-dev` for a mock-only preview build.

2. **Fixture state resolution timing**: The `?fixture=` param must be resolved synchronously before any React mount — specifically before the `callCommand` module is first imported, since the fixture state is read once at module init. If the resolver runs inside a React effect or provider, context providers will have already called `invoke()` with the wrong fixture state. Implementation must initialize the state in a module-scope `const` before the React tree mounts.

3. **Error fixture granularity**: Shell-critical commands (`settings_load`, `profile_list`, `profile_load`) must still resolve successfully in the `error` fixture state so the app shell renders. Only per-route data commands should reject. The `callCommand` dispatch table must distinguish shell-critical from route-level commands.
