// Profile optimization preset handlers: bundled presets, manual presets.
// See `lib/mocks/README.md`.
// All error messages MUST start with `[dev-mock]` to participate in the
// `.github/workflows/release.yml` "Verify no mock code in production bundle"
// sentinel.

import type { BundledOptimizationPreset, GameProfile } from '../../../types/profile';
import { emitMockEvent } from '../eventBus';
import { getStore } from '../store';
import { appendRevision } from './profile-history';
import { withProfileFixtureGate } from './profile-utils';
import type { Handler } from './types';

/** Same catalog as `profile_list_bundled_optimization_presets` — keep in sync for mock fidelity. */
function getBundledOptimizationPresets(): BundledOptimizationPreset[] {
  return [
    {
      preset_id: 'bundled/amd-fsr',
      display_name: 'AMD FSR Performance',
      vendor: 'amd',
      mode: 'performance',
      enabled_option_ids: ['use_gamemode', 'disable_vsync'],
      catalog_version: 1,
    },
    {
      preset_id: 'bundled/nvidia-dxvk',
      display_name: 'NVIDIA DXVK Quality',
      vendor: 'nvidia',
      mode: 'quality',
      enabled_option_ids: ['use_gamemode', 'enable_esync'],
      catalog_version: 1,
    },
  ];
}

export function registerProfilePresets(map: Map<string, Handler>): void {
  map.set(
    'profile_list_bundled_optimization_presets',
    withProfileFixtureGate(
      'profile_list_bundled_optimization_presets',
      async (): Promise<BundledOptimizationPreset[]> => getBundledOptimizationPresets()
    )
  );

  map.set(
    'profile_apply_bundled_optimization_preset',
    withProfileFixtureGate('profile_apply_bundled_optimization_preset', async (args) => {
      const { name, presetId } = args as { name: string; presetId: string };
      const trimmed = name.trim();
      const pid = presetId.trim();
      const store = getStore();
      const existing = store.profiles.get(trimmed);
      if (!existing) {
        throw new Error(`[dev-mock] profile_apply_bundled_optimization_preset: profile not found: ${trimmed}`);
      }
      const presets = getBundledOptimizationPresets();
      const normalizedPid = pid.startsWith('bundled/') ? pid : `bundled/${pid}`;
      const matched = presets.find((p) => p.preset_id === pid || p.preset_id === normalizedPid);
      const tomlKey = matched?.preset_id ?? normalizedPid;
      const presetIds = matched?.enabled_option_ids?.length ? [...matched.enabled_option_ids] : [];
      const updated: GameProfile = {
        ...existing,
        launch: {
          ...existing.launch,
          active_preset: tomlKey,
          presets: { ...(existing.launch.presets ?? {}), [tomlKey]: { enabled_option_ids: presetIds } },
          optimizations: { enabled_option_ids: presetIds },
        },
      };
      store.profiles.set(trimmed, updated);
      appendRevision(trimmed, 'preset_apply');
      emitMockEvent('profiles-changed', { name: trimmed, action: 'bundled-optimization-preset' });
      return structuredClone(updated);
    })
  );

  map.set(
    'profile_save_manual_optimization_preset',
    withProfileFixtureGate('profile_save_manual_optimization_preset', async (args) => {
      const { name, presetName, enabledOptionIds } = args as {
        name: string;
        presetName: string;
        enabledOptionIds: string[];
      };
      const trimmed = name.trim();
      const key = presetName.trim();
      const store = getStore();
      const existing = store.profiles.get(trimmed);
      if (!existing) {
        throw new Error(`[dev-mock] profile_save_manual_optimization_preset: profile not found: ${trimmed}`);
      }
      if (!key) {
        throw new Error('[dev-mock] profile_save_manual_optimization_preset: preset name must not be empty');
      }
      const ids = [...enabledOptionIds];
      const updated: GameProfile = {
        ...existing,
        launch: {
          ...existing.launch,
          active_preset: key,
          presets: { ...(existing.launch.presets ?? {}), [key]: { enabled_option_ids: ids } },
          optimizations: { enabled_option_ids: ids },
        },
      };
      store.profiles.set(trimmed, updated);
      appendRevision(trimmed, 'preset_apply');
      emitMockEvent('profiles-changed', { name: trimmed, action: 'manual-optimization-preset' });
      return structuredClone(updated);
    })
  );
}
