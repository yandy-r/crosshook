// src/crosshook-native/src/lib/fixture.ts
//
// Fixture-state switcher for browser dev mode (BR-11).
//
// Reads `?fixture=` from `window.location.search` exactly once at module init
// and caches the result in a module-scope const so subsequent calls are cheap.
//
// This module is intentionally tiny and dependency-free so it can be statically
// imported from BOTH production code paths (e.g. App.tsx) and the dev-only mock
// chunk (`lib/mocks/`) without dragging the mock registry into the production
// bundle. The fixture name itself is harmless to ship (it is just a string
// constant); the dispatch logic that ACTS on it lives inside `lib/mocks/`,
// which Rollup tree-shakes out of production.

/** Valid fixture states. Unknown query values fall back to `populated`. */
export type FixtureState = 'populated' | 'empty' | 'error' | 'loading';

const VALID_FIXTURES: ReadonlySet<FixtureState> = new Set(['populated', 'empty', 'error', 'loading']);

function readFixtureFromUrl(): FixtureState {
  // Defensive: avoid touching `window` in environments where it does not exist
  // (e.g. SSR, Node-side type tests). The browser dev path always has it.
  if (typeof window === 'undefined' || typeof window.location === 'undefined') {
    return 'populated';
  }
  const raw = new URLSearchParams(window.location.search).get('fixture');
  if (raw !== null && (VALID_FIXTURES as ReadonlySet<string>).has(raw)) {
    return raw as FixtureState;
  }
  return 'populated';
}

const ACTIVE_FIXTURE: FixtureState = readFixtureFromUrl();

/**
 * Returns the active fixture state, parsed once at module init from
 * `?fixture=` in the URL. Defaults to `'populated'` for any missing or
 * invalid value. Calls are O(1).
 */
export function getActiveFixture(): FixtureState {
  return ACTIVE_FIXTURE;
}
