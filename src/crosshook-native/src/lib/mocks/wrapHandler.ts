// src/crosshook-native/src/lib/mocks/wrapHandler.ts
//
// Orthogonal debug-toggle middleware for the mock handler registry (BR-12).
//
// Wraps every registered handler with two pieces of cross-cutting behavior
// that are independent from the per-handler fixture dispatch in `./fixture.ts`:
//
//   1. `?delay=<ms>` — adds `setTimeout(<ms>)` artificial latency before the
//      wrapped handler runs. Stacks with `?fixture=loading` correctly because
//      a never-resolving promise inside the handler trumps the upstream delay.
//
//   2. `?errors=true` — rejects mutating commands with a synthetic
//      `[dev-mock] forced error` message. Reads ALWAYS succeed so the app
//      shell can render and exercise the failure UI for write paths only.
//
// The `?onboarding=show` toggle is NOT handled here; it is an event-emission
// side effect implemented inline in `handlers/onboarding.ts` because it must
// fire once at module init, not on every handler invocation.

import { getActiveToggles } from '../toggles';
import type { Handler } from './handlers/types';

/**
 * Shell-critical reads that MUST resolve under every fixture/toggle combination
 * per BR-11. These commands are exempt from `?errors=true` rejection so the
 * `<AppShell />` always mounts.
 */
export const SHELL_CRITICAL_READS: ReadonlySet<string> = new Set([
  'settings_load',
  'recent_files_load',
  'default_steam_client_install_path',
  'profile_list',
  'profile_list_summaries',
]);

/**
 * Additional read-only commands enumerated explicitly so they are never
 * accidentally rejected by `?errors=true`. The list does NOT need to be
 * exhaustive — `isReadCommand()` falls back to a verb/noun regex heuristic
 * for any command not present here.
 */
const EXPLICIT_READ_COMMANDS: ReadonlySet<string> = new Set<string>([
  ...SHELL_CRITICAL_READS,
  'profile_load',
  'profile_list_favorites',
  'profile_list_bundled_optimization_presets',
  'profile_config_history',
  'profile_config_diff',
  'profile_export_toml',
  'check_game_running',
  'check_gamescope_session',
  'launch_platform_status',
  'check_readiness',
  // `check_generalized_readiness` is omitted: the real command persists a host readiness
  // snapshot via `upsert_host_readiness_snapshot` (see onboarding.rs), so it is not a
  // pure read. It still matches `READ_VERB_RE` (`check_` prefix) for test rendering.
  'preview_launch',
  'validate_launch',
  'build_steam_launch_options_command',
  'get_trainer_guidance',
  'verify_trainer_hash',
  // `collection_*` prefix doesn't match the `get_`-prefix regex, so list reads
  // explicitly. Phase 1 collection reads (`collection_list`, `collection_list_profiles`,
  // `collections_for_profile`) match the `_list`/`_list_*` noun-suffix regex; only
  // Phase 3's `collection_get_defaults` needs an explicit entry here.
  'collection_get_defaults',
  // Phase 4: preview-only import (mutation-sounding name).
  'collection_import_from_toml',
]);

/**
 * Heuristic: command names starting with common read-verb prefixes or ending
 * with common read-noun suffixes are treated as read-only unless overridden.
 * This is a deliberate allow-list bias because false-positives here only
 * weaken `?errors=true` (a contributor tool), while false-negatives would
 * crash shell reads.
 */
const READ_VERB_RE = /^(get_|list_|load_|read_|fetch_|check_|peek_|preview_|validate_|build_|verify_)/;
const READ_NOUN_RE = /(_load|_list|_summaries|_get|_status|_info|_state|_snapshot|_history|_diff|_export)$/;

/** Returns true when the command name should be exempt from `?errors=true`. */
export function isReadCommand(name: string): boolean {
  if (EXPLICIT_READ_COMMANDS.has(name)) return true;
  if (READ_VERB_RE.test(name)) return true;
  if (READ_NOUN_RE.test(name)) return true;
  return false;
}

/** Returns true for the BR-11 shell-critical read set. */
export function isShellCritical(name: string): boolean {
  return SHELL_CRITICAL_READS.has(name);
}

/**
 * Wraps a single handler with the orthogonal debug middleware:
 *
 * 1. If `?errors=true` AND the command is not a read, reject immediately.
 *    Shell-critical reads (BR-11) are always exempt — they fall through the
 *    `isReadCommand()` filter below.
 *
 * 2. If `?delay=<ms>` is set, await `setTimeout(<ms>)` BEFORE invoking the
 *    underlying handler. This means a handler that returns a never-resolving
 *    promise (e.g. fixture=loading) will still hang forever after the delay,
 *    preserving the loading-state behavior.
 */
export function wrapHandler(name: string, handler: Handler): Handler {
  return async (args: unknown): Promise<unknown> => {
    const toggles = getActiveToggles();

    if (toggles.forceErrors && !isReadCommand(name)) {
      throw new Error(`[dev-mock] forced error for ${name} (?errors=true)`);
    }

    if (toggles.delayMs > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, toggles.delayMs));
    }

    return handler(args);
  };
}

/**
 * Applies `wrapHandler()` to every entry in the registry map in place and
 * returns the same map reference for chaining. Call this from `registerMocks()`
 * AFTER every `register*()` call has populated the map.
 */
export function wrapAllHandlers(map: Map<string, Handler>): Map<string, Handler> {
  for (const [name, handler] of map.entries()) {
    map.set(name, wrapHandler(name, handler));
  }
  return map;
}
