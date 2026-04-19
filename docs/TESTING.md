# Testing Trophy and Canonical Patterns

This doc is the single entry point for frontend testing in CrossHook. It maps the testing trophy to
the commands in this repo and shows three ready-to-copy patterns (hook, component with
`user-event`, wizard/page flow).

- Source root: `src/crosshook-native/`
- Runner: **Vitest 4** (`happy-dom`)
- UI helper stack: **React Testing Library** + **user-event**
- IPC mocking: `vi.mock('@/lib/ipc', ...)` → `registerMocks()` handlers
- E2E smoke: **Playwright** against `vite --mode webdev`

## Quick commands

From `src/crosshook-native/`:

```bash
npm test                 # Vitest (happy-dom)
npm run test:watch       # Vitest watch
npm run test:coverage    # Vitest + V8 coverage
npm run test:smoke       # Playwright smoke (browser dev mode)
npm run test:smoke:update
npm run test:smoke:install
```

## Testing trophy

- **Unit / hooks / small components (wide base)** — Vitest + RTL in happy-dom. Use the IPC mock
  harness so tests exercise real handlers instead of hand-written stubs.
- **Mock-driven flows (middle)** — Still Vitest, but wire real handlers and `user-event` to drive
  wizard/page flows without launching the app.
- **E2E smoke (thin top)** — Playwright `test:smoke` hits `vite --mode webdev`. Proves routing,
  dev-mode chip, and console hygiene. It is not a WebKitGTK parity test; always re-verify with
  `./scripts/dev-native.sh` before merging UI changes.

## Pattern library

### Hook test (IPC-backed)

Use the IPC mock bridge so hook calls go through the same handlers as browser dev mode.

```ts
// src/hooks/__tests__/useOnboarding.test.ts
import { renderHook, act, waitFor } from '@testing-library/react';
import { configureMockHandlers, mockCallCommand } from '@/test/render';
import { makeReadinessResult } from '@/test/fixtures';

vi.mock('@/lib/ipc', () => ({ callCommand: mockCallCommand }));

beforeEach(() => configureMockHandlers());

it('stores readiness results after runChecks resolves', async () => {
  configureMockHandlers({
    handlerOverrides: {
      check_generalized_readiness: async () => makeReadinessResult({ all_passed: true, critical_failures: 0 }),
    },
  });

  const { result } = renderHook(() => useOnboarding());

  await act(async () => result.current.runChecks());
  await waitFor(() => expect(result.current.readinessResult?.all_passed).toBe(true));
});
```

- `configureMockHandlers()` seeds a fresh handler map and resets singleton mock state.
- `handlerOverrides` overrides individual commands without redefining the whole registry.
- Prefer `renderHook` + `waitFor` over manual promise resolution so state updates flush reliably.

### Component test with `user-event`

Wire the IPC bridge once, then drive the UI with realistic input (keyboard, focus, pointer).

```tsx
// src/components/library/__tests__/LibraryCard.test.tsx
import userEvent from '@testing-library/user-event';
import { renderWithMocks } from '@/test/render';
import { triggerIntersection } from '@/test/setup';
import { makeLibraryCardData } from '@/test/fixtures';

vi.mock('@/lib/ipc', () => ({ callCommand: mockCallCommand }));

it('loads cover art after the card enters the viewport', async () => {
  renderWithMocks(<LibraryCard profile={makeLibraryCardData()} {...callbacks} />, {
    handlerOverrides: {
      fetch_game_cover_art: async () => '/mock/media/synthetic-quest.png',
    },
  });

  triggerIntersection(screen.getByRole('listitem'), true);
  await waitFor(() =>
    expect(screen.getByRole('img', { name: 'Synthetic Quest' })).toHaveAttribute(
      'src',
      '/mock/media/synthetic-quest.png'
    )
  );
});

it('opens the context menu from keyboard', async () => {
  const user = userEvent.setup();
  renderWithMocks(<LibraryCard profile={makeLibraryCardData()} {...callbacks} />);
  const detailsButton = screen.getByRole('button', { name: /View details/ });

  detailsButton.focus();
  await user.keyboard('{Shift>}{F10}{/Shift}');

  expect(callbacks.onContextMenu).toHaveBeenCalled();
});
```

- `renderWithMocks` creates handler map + resets mock store; pass `handlerOverrides` for per-test data.
- `triggerIntersection` comes from `src/test/setup.ts` and drives lazy-load paths.
- Prefer `userEvent` over `fireEvent` for keyboard/focus realism; fall back to `fireEvent` only for
  low-level DOM events.

### Wizard/page flow

Stub context providers + expensive hooks, then drive the flow with `userEvent`. This keeps the flow
focused while avoiding real IPC for everything.

```tsx
// src/components/__tests__/OnboardingWizard.test.tsx
const useOnboardingMock = vi.fn();
vi.mock('@/hooks/useOnboarding', () => ({ useOnboarding: () => useOnboardingMock() }));
// ...other context mocks...

beforeEach(() => {
  useOnboardingMock.mockReturnValue({
    stage: 'identity_game',
    dismiss: vi.fn().mockResolvedValue(undefined),
    // ...
  });
});

it('dismisses onboarding before invoking onDismiss from Skip Setup', async () => {
  const dismiss = vi.fn().mockResolvedValue(undefined);
  useOnboardingMock.mockReturnValue({ ...baseState, dismiss });

  render(<OnboardingWizard open onComplete={vi.fn()} onDismiss={onDismiss} />);
  await user.click(screen.getByRole('button', { name: 'Skip Setup' }));

  await waitFor(() => expect(onDismiss).toHaveBeenCalled());
  expect(dismiss).toHaveBeenCalled();
});
```

- Mock only the outer boundaries (contexts, cross-cutting hooks) to keep assertions readable.
- Keep validations realistic: return `evaluateWizardRequiredFields` data the UI expects instead of
  stubbing `true`/`false` randomly.
- When a flow depends on IPC data, prefer wiring `renderWithMocks` + handler overrides rather than
  mocking `callCommand` per test.

## IPC mocking: when to use what

- **Default**: `vi.mock('@/lib/ipc', { callCommand: mockCallCommand })` + `renderWithMocks` or
  `configureMockHandlers`. This dispatches into the registry in `src/lib/mocks/index.ts`, the same
  handlers browser dev mode uses. Seed data with `seed` or `handlerOverrides`.
- **`mockIPC` (tauri API)**: reserve for testing `src/lib/ipc.ts` itself (the adapter). Example:
  `src/lib/__tests__/ipc.test.ts` uses `vi.doMock('@tauri-apps/api/mocks', ...)` to prove the
  adapter calls `@tauri-apps/api/core` when `isTauri()` is true. Do **not** use `mockIPC` for
  component/hook tests.

## Pitfalls and resets

- **Module-init toggles**: `?fixture=`, `?delay=`, `?errors=`, `?onboarding=` are read once in
  `src/lib/fixture.ts` / `src/lib/mocks/wrapHandler.ts`. To swap fixtures inside a test file, mock
  those modules (`vi.mock('@/lib/fixture', ...)`) or `vi.resetModules()` before importing the code
  under test.
- **Mock store singletons**: `registerMocks()` uses a singleton store in `src/lib/mocks/store.ts`.
  `renderWithMocks` + `configureMockHandlers` call `resetMockEnvironment()` and `resetMockHandlers()`
  so every test starts clean. Avoid hanging onto handler maps between tests.
- **Timers and observers**: `src/test/setup.ts` installs fake `IntersectionObserver`, RAF, and
  `matchMedia`, and clears them in `afterEach`. Use helpers like `triggerIntersection` instead of
  reimplementing them.
- **Fixture data**: use builders in `src/test/fixtures.ts` (e.g. `makeLibraryCardData`,
  `makeReadinessResult`, `makeProfileDraft`) to stay within the synthetic data policy (no real game
  names or paths).

## Where things live

- Config: `src/crosshook-native/vitest.config.ts`
- Helpers: `src/crosshook-native/src/test/{setup.ts,render.tsx,fixtures.ts}`
- Tests: co-located `__tests__/` folders under `src/hooks`, `src/components`, `src/lib`, …
- Smoke: `src/crosshook-native/tests/` (Playwright) — see `tests/README.md`

If you add a new test type, keep helpers under `src/test/` and reference them here.
