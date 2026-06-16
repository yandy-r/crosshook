import type { GameProfile, SerializedGameProfile } from '../../types';
import type { LaunchCommandArguments } from '../../types/launch-command-arguments';
import type { LaunchOptimizations } from '../../types/launch-optimizations';
import { normalizeInjectionSection, normalizeSerializedGameProfile } from '../../types/profile';
import { resolveLaunchMethod } from '../../utils/launch';
import type { OptimizationEntry } from '../../utils/optimization-catalog';
import { normalizeLaunchOptimizationIds } from './launchOptimizationIds';
import { deriveGameName, deriveLauncherDisplayName, stripAutomaticLauncherSuffix } from './profileDisplayNames';

const UMU_RUNTIME_HINT_MAX_LENGTH = 128;

function normalizeCommandArgumentIds(ids: readonly string[] | undefined): string[] {
  const normalized: string[] = [];
  const seenIds = new Set<string>();

  for (const argumentId of ids ?? []) {
    const trimmedId = argumentId.trim();
    if (!trimmedId || seenIds.has(trimmedId)) {
      continue;
    }
    seenIds.add(trimmedId);
    normalized.push(trimmedId);
  }

  return normalized;
}

function normalizeCommandArgumentCustomArgsForEdit(args: readonly string[] | undefined): string[] {
  return (args ?? []).map((arg) => arg.trim());
}

function dropBlankCommandArgumentCustomArgs(args: readonly string[]): string[] {
  return args.filter((arg) => arg.trim().length > 0);
}

function normalizeCommandArgumentsForEdit(
  commandArguments: LaunchCommandArguments | undefined
): LaunchCommandArguments {
  return {
    enabled_argument_ids: normalizeCommandArgumentIds(commandArguments?.enabled_argument_ids),
    custom_args: normalizeCommandArgumentCustomArgsForEdit(commandArguments?.custom_args),
  };
}

function normalizeCommandArgumentsForSave(
  commandArguments: LaunchCommandArguments | undefined
): LaunchCommandArguments {
  const forEdit = normalizeCommandArgumentsForEdit(commandArguments);
  return {
    enabled_argument_ids: forEdit.enabled_argument_ids,
    custom_args: dropBlankCommandArgumentCustomArgs(forEdit.custom_args),
  };
}

function normalizeUmuRuntimeHint(value: string | undefined, options: { lowercase: boolean }): string {
  const trimmed = (value ?? '').trim();
  if ([...trimmed].some((character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127)) {
    return '';
  }
  const capped = trimmed.slice(0, UMU_RUNTIME_HINT_MAX_LENGTH);
  return options.lowercase ? capped.toLowerCase() : capped;
}

function normalizeLaunchPresetsSection(
  profile: GameProfile,
  optionsById: Record<string, OptimizationEntry>,
  catalogLoaded: boolean
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
        enabled_option_ids: normalizeLaunchOptimizationIds(value?.enabled_option_ids, optionsById, catalogLoaded),
      };
    }
  }
  const active_preset = (profile.launch.active_preset ?? '').trim();
  return { presets, active_preset };
}

export function normalizeProfileForEdit(
  profile: SerializedGameProfile | GameProfile,
  optionsById: Record<string, OptimizationEntry>,
  catalogLoaded: boolean
): GameProfile {
  const normalizedProfile = normalizeSerializedGameProfile(profile);
  const method = resolveLaunchMethod(normalizedProfile);
  const runtime = normalizedProfile.runtime;
  const { presets, active_preset } = normalizeLaunchPresetsSection(normalizedProfile, optionsById, catalogLoaded);
  let enabledOptionIds = normalizeLaunchOptimizationIds(
    normalizedProfile.launch.optimizations?.enabled_option_ids,
    optionsById,
    catalogLoaded
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
    injection: normalizeInjectionSection(normalizedProfile.injection),
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
      umu_store: normalizeUmuRuntimeHint(runtime.umu_store, { lowercase: true }),
      umu_codename: normalizeUmuRuntimeHint(runtime.umu_codename, { lowercase: false }),
    },
    launch: {
      ...normalizedProfile.launch,
      method,
      presets,
      active_preset,
      optimizations: {
        enabled_option_ids: enabledOptionIds,
      },
      command_arguments: normalizeCommandArgumentsForEdit(normalizedProfile.launch.command_arguments),
      custom_env_vars: { ...(normalizedProfile.launch.custom_env_vars ?? {}) },
    },
  };
}

export function normalizeProfileForSave(
  profile: GameProfile,
  optionsById: Record<string, OptimizationEntry>,
  catalogLoaded: boolean
): GameProfile {
  const normalized = normalizeProfileForEdit(profile, optionsById, catalogLoaded);

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
    launch: {
      ...normalized.launch,
      command_arguments: normalizeCommandArgumentsForSave(normalized.launch.command_arguments),
    },
  };
}
