/**
 * Shared helpers for addressing bundled GPU optimization presets under
 * `[launch.presets]` in profile TOML. Mirrors `bundled_optimization_preset_toml_key`
 * in crosshook-core. Keep this file the single source of truth so the wizard
 * preset picker and the full Launch Optimizations panel cannot drift.
 */

/** Prefix under `[launch.presets]` for bundled catalog presets. */
export const BUNDLED_PRESET_KEY_PREFIX = 'bundled/';

/** Builds the `[launch.presets.<key>]` key for a bundled catalog preset id. */
export function bundledOptimizationTomlKey(presetId: string): string {
  return `${BUNDLED_PRESET_KEY_PREFIX}${presetId.trim()}`;
}

/** Returns true when the given preset key maps to a bundled catalog preset. */
export function isBundledOptimizationPresetKey(key: string): boolean {
  return key.startsWith(BUNDLED_PRESET_KEY_PREFIX);
}

/** Extracts the bundled preset id from its `[launch.presets]` key, or null. */
export function extractBundledOptimizationPresetId(key: string): string | null {
  if (!isBundledOptimizationPresetKey(key)) {
    return null;
  }
  return key.slice(BUNDLED_PRESET_KEY_PREFIX.length);
}
