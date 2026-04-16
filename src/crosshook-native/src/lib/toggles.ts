// src/crosshook-native/src/lib/toggles.ts
//
// Orthogonal debug toggles for browser dev mode (BR-12).
//
// Reads `?delay=`, `?errors=`, and `?onboarding=` from `window.location.search`
// exactly once at module init and caches the parsed result in a module-scope
// const so subsequent calls are O(1).
//
// These toggles are independent from the `?fixture=` switcher in `./fixture.ts`
// and can be combined freely. For example:
//
//   ?fixture=empty&delay=800            empty data + 800ms artificial latency
//   ?errors=true&delay=200              mutation errors + 200ms latency
//   ?fixture=error&onboarding=show      error state + synthesized onboarding event
//
// Like `./fixture.ts`, this module is intentionally tiny and dependency-free so
// it can be statically imported from BOTH production code paths (e.g. `App.tsx`)
// and the dev-only mock chunk (`lib/mocks/`) without dragging the mock registry
// into the production bundle. The toggle values themselves are harmless to
// ship; the dispatch logic that ACTS on them lives inside `lib/mocks/` (chiefly
// `wrapHandler.ts`), which Rollup tree-shakes out of production.

/** Parsed orthogonal debug toggles. */
export interface DebugToggles {
  /** If > 0, every mock handler resolves after `delayMs` (via `setTimeout`). */
  delayMs: number;
  /** If true, every mutating command rejects with a synthetic error. Reads always succeed. */
  forceErrors: boolean;
  /** If true, synthesize an `onboarding-check` event at module init. */
  showOnboarding: boolean;
  /** If true, populate `steam_deck_caveats` in `check_readiness` responses (when not yet dismissed). */
  showSteamDeckCaveats: boolean;
}

function safeReadSearchParams(): URLSearchParams {
  // Defensive: avoid touching `window` in environments where it does not exist
  // (e.g. SSR, Node-side type tests). The browser dev path always has it.
  if (typeof window === 'undefined' || typeof window.location === 'undefined') {
    return new URLSearchParams();
  }
  return new URLSearchParams(window.location.search);
}

function parseToggles(): DebugToggles {
  const params = safeReadSearchParams();
  const rawDelay = params.get('delay');
  const parsedDelay = rawDelay !== null ? Number.parseInt(rawDelay, 10) : 0;
  const delayMs = Number.isFinite(parsedDelay) && parsedDelay > 0 ? parsedDelay : 0;
  const forceErrors = params.get('errors') === 'true';
  const showOnboarding = params.get('onboarding') === 'show';
  const showSteamDeckCaveats = params.get('steamDeckCaveats') === 'show';
  return { delayMs, forceErrors, showOnboarding, showSteamDeckCaveats };
}

const ACTIVE_TOGGLES: DebugToggles = parseToggles();

/**
 * Returns the active debug toggles, parsed once at module init from
 * `?delay=`, `?errors=`, and `?onboarding=` in the URL. Calls are O(1).
 */
export function getActiveToggles(): DebugToggles {
  return ACTIVE_TOGGLES;
}

/**
 * Returns human-readable label fragments for the dev-mode chip. Order is
 * deterministic so the chip label is stable across reloads with the same
 * URL. Returns an empty array when no toggles are active.
 */
export function togglesToChipFragments(toggles: DebugToggles): readonly string[] {
  const fragments: string[] = [];
  if (toggles.forceErrors) fragments.push('errors');
  if (toggles.delayMs > 0) fragments.push(`${toggles.delayMs}ms`);
  if (toggles.showOnboarding) fragments.push('onboarding');
  if (toggles.showSteamDeckCaveats) fragments.push('steamDeckCaveats');
  return fragments;
}
