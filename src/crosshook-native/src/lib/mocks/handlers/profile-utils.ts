// Shared utilities for profile mock handlers. See `lib/mocks/README.md`.
// All error messages MUST start with `[dev-mock]` to participate in the
// `.github/workflows/release.yml` "Verify no mock code in production bundle"
// sentinel.

import type { GameProfile } from '../../../types/profile';
import { getActiveFixture } from '../../fixture';
import type { Handler } from './types';

// ---------------------------------------------------------------------------
// Fixture helpers (BR-11)
// ---------------------------------------------------------------------------
//
// `populated` (default) — current behavior, returns demo data
// `empty`               — list/load handlers return empty/null
// `error`               — fallible handlers throw; shell-critical reads still
//                          resolve so `<AppShell />` can mount
// `loading`             — non-shell-critical handlers never resolve
//                          (`new Promise(() => {})`); shell-critical reads
//                          still resolve so the shell renders
//
// NOTE: These helpers are NOT subsumed by the `wrapHandler()` middleware
// added in Task 3.2 (`lib/mocks/wrapHandler.ts`). The middleware implements
// the orthogonal `?errors=true` / `?delay=<ms>` toggles, while these helpers
// implement the per-handler `?fixture=loading|error` dispatch. The two
// systems are deliberately independent — keep both.

/**
 * Returns a promise that never resolves. Used by the `loading` fixture so
 * loading-state UIs (skeletons, spinners) stay visible during dev review.
 * Orthogonal to the `?delay=<ms>` toggle in `wrapHandler.ts`.
 */
export function neverResolving<T>(): Promise<T> {
  return new Promise<T>(() => {
    /* intentionally never resolves */
  });
}

/**
 * Synthesizes a `[dev-mock] forced error` for the named command. Used by the
 * `?fixture=error` dispatch path. Orthogonal to the `?errors=true` toggle in
 * `wrapHandler.ts` — fixture-error throws even for reads, while the toggle
 * exempts reads via `isReadCommand()`.
 */
export function forcedError(commandName: string): Error {
  return new Error(`[dev-mock] forced error for ${commandName}`);
}

/**
 * Standard `?fixture=error|loading` gate for profile mutators and non-shell reads.
 * Shell-critical handlers (`profile_list`, `profile_load`, …) keep bespoke logic.
 */
export function withProfileFixtureGate(commandName: string, impl: Handler): Handler {
  return async (args: unknown) => {
    const fixture = getActiveFixture();
    if (fixture === 'error') throw forcedError(commandName);
    if (fixture === 'loading') return neverResolving<unknown>();
    return impl(args);
  };
}

// ---------------------------------------------------------------------------
// Collection defaults merge layer
// ---------------------------------------------------------------------------

export interface MockCollectionDefaults {
  method?: string;
  optimizations?: { enabled_option_ids: string[] };
  custom_env_vars?: Record<string, string>;
  network_isolation?: boolean;
  gamescope?: unknown;
  trainer_gamescope?: unknown;
  mangohud?: unknown;
}

/**
 * Apply collection defaults to a loaded profile (browser dev-mode parity for the
 * Rust `effective_profile_with` merge layer). Mirrors the precedence: collection
 * defaults override profile base for the editable subset, but `custom_env_vars` is
 * an additive merge where collection keys win on collision.
 *
 * Excludes the `local_override` layer because the dev-mode profile fixtures do not
 * carry one — production Rust applies it after the collection layer.
 */
export function applyMockCollectionDefaults(profile: GameProfile, d: MockCollectionDefaults): GameProfile {
  // Deep-clone via JSON round-trip — GameProfile is serde-friendly (no cycles,
  // no Date, no undefined surprises) so this is the safest portable clone.
  const merged: GameProfile = JSON.parse(JSON.stringify(profile));

  if (typeof d.method === 'string' && d.method.trim() !== '') {
    // Caller (Rust serde) only emits known LaunchMethod variants; mock relays as-is.
    merged.launch.method = d.method as GameProfile['launch']['method'];
  }
  if (d.optimizations) {
    merged.launch.optimizations = {
      ...merged.launch.optimizations,
      enabled_option_ids: [...(d.optimizations.enabled_option_ids ?? [])],
    };
  }
  if (d.custom_env_vars) {
    merged.launch.custom_env_vars = {
      ...(merged.launch.custom_env_vars ?? {}),
      ...d.custom_env_vars, // collection wins on collision
    };
  }
  if (typeof d.network_isolation === 'boolean') {
    merged.launch.network_isolation = d.network_isolation;
  }
  if (d.gamescope !== undefined) {
    (merged.launch as { gamescope: unknown }).gamescope = d.gamescope;
  }
  if (d.trainer_gamescope !== undefined) {
    (merged.launch as { trainer_gamescope: unknown }).trainer_gamescope = d.trainer_gamescope;
  }
  if (d.mangohud !== undefined) {
    (merged.launch as { mangohud: unknown }).mangohud = d.mangohud;
  }

  return merged;
}
