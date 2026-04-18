import type { GameProfile } from '../../types';
import { getConflictingLaunchOptimizationIds, type LaunchOptimizationId } from '../../types/launch-optimizations';
import type { OptimizationEntry } from '../../utils/optimization-catalog';
import { normalizeLaunchOptimizationIds } from './launchOptimizationIds';

export type ApplyLaunchOptimizationToggleResult =
  | { ok: true; profile: GameProfile }
  | { ok: false; conflictLabels: string[] };

export function applyLaunchOptimizationToggle(
  current: GameProfile,
  optionId: LaunchOptimizationId,
  nextEnabled: boolean,
  optionsById: Record<string, OptimizationEntry>,
  conflictMatrix: Readonly<Record<string, readonly string[]>>,
  catalogLoaded: boolean
): ApplyLaunchOptimizationToggleResult {
  const currentIds = current.launch.optimizations.enabled_option_ids;
  const conflictingIds = nextEnabled ? getConflictingLaunchOptimizationIds(optionId, currentIds, conflictMatrix) : [];

  if (conflictingIds.length > 0) {
    const conflictLabels = conflictingIds.map((conflictingId) => optionsById[conflictingId]?.label ?? conflictingId);
    return { ok: false, conflictLabels };
  }

  const nextIds = nextEnabled
    ? normalizeLaunchOptimizationIds([...currentIds, optionId], optionsById, catalogLoaded)
    : currentIds.filter((currentOptionId) => currentOptionId !== optionId);

  const activeKey = (current.launch.active_preset ?? '').trim();
  const presets = { ...(current.launch.presets ?? {}) };
  if (activeKey && presets[activeKey]) {
    presets[activeKey] = { enabled_option_ids: nextIds };
  }

  return {
    ok: true,
    profile: {
      ...current,
      launch: {
        ...current.launch,
        presets,
        optimizations: {
          enabled_option_ids: nextIds,
        },
      },
    },
  };
}
