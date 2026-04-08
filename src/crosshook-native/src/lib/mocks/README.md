# Mock Registry — Contributor Guide

This directory implements the browser-mode IPC mock layer used by the `dev:browser`
Vite dev mode (`--mode webdev`). It intercepts every `callCommand()` call that would
otherwise hit a real Tauri backend, and returns synthetic in-memory responses.

See the feature spec for the full business rules:
[feature-spec.md](../../../../../../docs/plans/dev-web-frontend/feature-spec.md)
(BR-1 through BR-12).

---

## Architecture

```
lib/mocks/
├── index.ts          — orchestrator: builds and exports the handler Map
├── store.ts          — singleton in-memory state (settings, profiles, …)
├── eventBus.ts       — re-exports emitMockEvent from lib/events.ts
├── handlers/
│   ├── settings.ts   — settings_* and recent_files_* commands
│   ├── profile.ts    — profile_* lifecycle + mutations + config history
│   ├── launch.ts     — launch_*, preview/validate, gamescope session probe
│   ├── install.ts    — install_game and install-* event sequence
│   ├── update.ts     — update_game, cancel_update, update-complete event
│   ├── health.ts     — batch_validate_profiles, version status, snapshot cache
│   ├── onboarding.ts — readiness check, trainer guidance, onboarding-check event
│   ├── proton.ts     — Proton install discovery and migrations
│   ├── protonup.ts   — ProtonUp catalog + install (no real download)
│   ├── protondb.ts   — ProtonDB lookup + suggestions + dismissal tracking
│   ├── community.ts  — community profile index, taps, sync, import/export
│   ├── launcher.ts   — desktop launcher export, delete, rename, preview
│   ├── library.ts    — cover art fetch, Steam metadata auto-populate
│   └── system.ts     — discovery, run-executable, prefix storage/deps,
│                       diagnostics, optimization catalog, offline readiness
└── README.md         — this file
```

**Total commands registered**: ~113 across 14 handler files, covering ~95% of
the Rust `#[tauri::command]` surface. The uncovered commands (`collection_*`)
have no frontend callers yet, so they are intentionally not mocked.

`ipc.ts` dynamically imports `./mocks` and calls `registerMocks()` when running in
`webdev` mode. The returned `Map<string, Handler>` is used to dispatch every
`callCommand()` invocation.

---

## How to Add a New Handler Domain

1. Create `handlers/<domain>.ts`:

```ts
import type { Handler } from '../index';
import { getStore } from '../store';

export function register<Domain>(map: Map<string, Handler>): void {
  map.set('some_command', (_args) => {
    const store = getStore();
    return store.someField;
  });
}
```

2. Import and call it in `index.ts`:

```ts
import { registerDomain } from './handlers/<domain>';

export function registerMocks(): Map<string, Handler> {
  const map = new Map<string, Handler>();
  // …existing registrations…
  registerDomain(map);
  return map;
}
```

3. Add any new state fields to `MockStore` in `store.ts` and seed them in
   `getStore()`.

Error prefix: use `[dev-mock]` in all thrown errors so they are identifiable in
the browser console, e.g.:

```ts
throw new Error('[dev-mock] profile not found: ' + id);
```

---

## The `getStore()` Singleton

`store.ts` exports a lazy singleton that is initialised on first call:

```ts
import { getStore, resetStore } from '../store';

const store = getStore(); // initialises on first access
resetStore();             // wipes state; next getStore() returns a fresh copy
```

**HMR behaviour**: Vite HMR resets the store on any handler edit — you get clean
state on save. This is intentional: mocks are fixtures, not persistent state.
If you need a specific starting state for manual testing, seed it directly in
`getStore()` or add a `?fixture=<name>` switcher (see Phase 3 scope below).

---

## Fixture Content Policy

All synthetic data must satisfy these constraints (BR-7):

- **Game names**: fictional or clearly synthetic — e.g. `"Synthetic Quest"`,
  `"Dev Test Game"`. No real game titles.
- **Steam App IDs**: use values ≥ `9999001` to avoid collisions with real Steam
  catalog entries.
- **File paths**: use `/home/devuser/…` or `/mock/…` prefixes. No real system
  paths.
- **No real network activity**: mocks must never make HTTP requests or access
  the filesystem.

---

## `?fixture=` URL Switcher

Select a named fixture scenario via the URL query parameter `?fixture=<name>`.
Parsed once at module init in `lib/fixture.ts` (so a reload is required to
change it) and dispatched per-handler. Unknown values fall back to `populated`.

| Value         | Behavior                                                                       |
| ------------- | ------------------------------------------------------------------------------ |
| `populated`   | Default. Returns demo data with seeded profiles, settings, and history.        |
| `empty`       | Empty arrays / `null` for list/load handlers. Mutations still succeed.         |
| `error`       | Fallible commands throw `[dev-mock] forced error`. Shell-critical reads still resolve (BR-11). |
| `loading`     | Non-shell-critical handlers return a never-resolving promise so loading UIs stay visible. |

The chip label reflects the active fixture, e.g. `DEV · empty`.

---

## Orthogonal Debug Toggles (BR-12)

Three orthogonal toggles can be combined freely with each other AND with the
fixture switcher above. Like `?fixture=`, they are parsed once at module init
(in `lib/toggles.ts`) and a reload is required to change them.

| Toggle              | Effect                                                                                   |
| ------------------- | ---------------------------------------------------------------------------------------- |
| `?delay=<ms>`       | Adds `setTimeout(<ms>)` artificial latency before EVERY mock handler runs.               |
| `?errors=true`      | Rejects mutating commands with a synthetic `[dev-mock] forced error`. Reads succeed.    |
| `?onboarding=show`  | Synthesizes an `onboarding-check` event 500ms after mount so the wizard surfaces.        |

`?delay` and `?errors` are implemented as a `wrapHandler()` middleware in
`lib/mocks/wrapHandler.ts` that wraps every registered handler exactly once
during `registerMocks()`. `?onboarding=show` is a one-shot module-init side
effect inside `handlers/onboarding.ts`.

**Read detection**: `?errors=true` exempts read commands so the app shell can
render. `wrapHandler.ts` uses an explicit allow-list (`SHELL_CRITICAL_READS`
plus `EXPLICIT_READ_COMMANDS`) and a verb/noun regex heuristic
(`get_`, `list_`, `load_`, `_load`, `_list`, …). Add new shell-critical reads
to `SHELL_CRITICAL_READS` if they boot the app.

**Stacking with fixtures**: a never-resolving promise from `?fixture=loading`
trumps `?delay=<ms>` because the delay lives outside the handler — once the
delay finishes, the inner promise still hangs forever.

### Combination examples

```
http://localhost:5173/?fixture=empty&delay=800
        Empty data + 800ms artificial latency. Chip: DEV · empty · 800ms

http://localhost:5173/?errors=true&delay=200
        Mutation errors + 200ms latency. Chip: DEV · populated · errors · 200ms

http://localhost:5173/?fixture=error&onboarding=show
        Error fixture + synthesized onboarding event. Chip: DEV · error · onboarding

http://localhost:5173/?fixture=empty&errors=true&delay=400&onboarding=show
        All four. Chip: DEV · empty · errors · 400ms · onboarding
```

The dev-mode chip in the lower-right corner shows the active fixture and
toggle fragments separated by `·`. Order is deterministic
(`fixture, errors, delay, onboarding`) so screenshots are stable across
reloads with the same URL.

---

## Do Not Deploy Warning

`vite build --mode webdev` compiles the mock layer into the output bundle. If
that output were uploaded to a web host it would ship mock code as a public
website — this is an intentional foot-gun. The `dev:browser` script is only
meant for local development. Never publish the `webdev` build output.

---

## Cross-References

- `lib/ipc.ts` — dynamic import point; dispatches commands to this registry
- `lib/events.ts` — `emitMockEvent` source (re-exported by `eventBus.ts`)
- `lib/runtime.ts` — `isTauri()` guard used in `events.ts`
- `docs/plans/dev-web-frontend/feature-spec.md` — full spec including BR-1–BR-12
