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
│   ├── settings.ts   — IPC handlers for settings_* and recent_files_* commands
│   └── profile.ts    — IPC handlers for profile_* commands
└── README.md         — this file
```

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

## `?fixture=` URL Switcher (Phase 3 Scope)

Phase 3 will add support for selecting a named fixture set via the URL query
parameter `?fixture=<name>` (e.g. `?fixture=empty`, `?fixture=populated`). The
hook is pre-documented here so Phase 2 contributors can plan for it. The
`resetStore()` export in `store.ts` is the natural reset point for fixture
switching.

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
