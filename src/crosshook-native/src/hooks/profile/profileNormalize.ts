import type { GameProfile, SerializedGameProfile } from '../../types';
import type { LaunchOptimizations } from '../../types/launch-optimizations';
import { normalizeSerializedGameProfile } from '../../types/profile';
import { resolveLaunchMethod } from '../../utils/launch';
import type { OptimizationEntry } from '../../utils/optimization-catalog';
import { normalizeLaunchOptimizationIds } from './launchOptimizationIds';
import { deriveGameName, deriveLauncherDisplayName, stripAutomaticLauncherSuffix } from './profileDisplayNames';

function normalizeLaunchPresetsSection(
  profile: GameProfile,
  optionsById: Record<string, OptimizationEntry>
): {
  presets: Record<string, LaunchOptimizations>;
  active_preset: string;
} {
  const raw = profile.launch.presets;
  const presets: Record<string, LaunchOptimizations> = {};
  if (raw && typeof raw === 'object') {
    for (const [key, value] of Object.entries(raw)) {
      const name = key.trim();
      if (!name) {
        continue;
      }
      presets[name] = {
        enabled_option_ids: normalizeLaunchOptimizationIds(value?.enabled_option_ids, optionsById),
      };
    }
  }
  const active_preset = (profile.launch.active_preset ?? '').trim();
  return { presets, active_preset };
}

export function normalizeProfileForEdit(
  profile: SerializedGameProfile | GameProfile,
  optionsById: Record<string, OptimizationEntry>
): GameProfile {
  const normalizedProfile = normalizeSerializedGameProfile(profile);
  const method = resolveLaunchMethod(normalizedProfile);
  const runtime = normalizedProfile.runtime;
  const { presets, active_preset } = normalizeLaunchPresetsSection(normalizedProfile, optionsById);
  let enabledOptionIds = normalizeLaunchOptimizationIds(
    normalizedProfile.launch.optimizations?.enabled_option_ids,
    optionsById
  );
  if (active_preset && presets[active_preset]) {
    enabledOptionIds = presets[active_preset].enabled_option_ids;
  }

  return {
    ...normalizedProfile,
    trainer: {
      ...normalizedProfile.trainer,
      type: normalizedProfile.trainer.type.trim(),
      loading_mode: normalizedProfile.trainer.loading_mode ?? 'source_directory',
    },
    steam: {
      ...normalizedProfile.steam,
      enabled: method === 'steam_applaunch',
      launcher: {
        ...normalizedProfile.steam.launcher,
        display_name: stripAutomaticLauncherSuffix(normalizedProfile.steam.launcher.display_name),
      },
    },
    runtime: {
      ...runtime,
      prefix_path: runtime.prefix_path.trim(),
      proton_path: runtime.proton_path.trim(),
      working_directory: runtime.working_directory.trim(),
    },
    launch: {
      ...normalizedProfile.launch,
      method,
      presets,
      active_preset,
      optimizations: {
        enabled_option_ids: enabledOptionIds,
      },
      custom_env_vars: { ...(normalizedProfile.launch.custom_env_vars ?? {}) },
    },
  };
}

export function normalizeProfileForSave(
  profile: GameProfile,
  optionsById: Record<string, OptimizationEntry>
): GameProfile {
  const normalized = normalizeProfileForEdit(profile, optionsById);

  return {
    ...normalized,
    game: {
      ...normalized.game,
      name: deriveGameName(normalized),
    },
    trainer: {
      ...normalized.trainer,
    },
    steam: {
      ...normalized.steam,
      launcher: {
        ...normalized.steam.launcher,
        display_name: deriveLauncherDisplayName(normalized),
      },
    },
  };
}
