import type { ProtonDbRecommendationGroup } from '../types/protondb';
import type { GameProfile } from '../types/profile';
import type { OptimizationCatalogPayload } from './optimization-catalog';

export interface ProtonDbEnvVarConflict {
  key: string;
  currentValue: string;
  suggestedValue: string;
}

/** Pending user confirmation when ProtonDB suggestions conflict with existing custom env vars. */
export type PendingProtonDbOverwrite = {
  group: ProtonDbRecommendationGroup;
  conflicts: ProtonDbEnvVarConflict[];
  resolutions: Record<string, 'keep_current' | 'use_suggestion'>;
};

export interface ProtonDbEnvVarMergeResult {
  mergedEnvVars: Record<string, string>;
  conflicts: ProtonDbEnvVarConflict[];
  appliedKeys: string[];
  unchangedKeys: string[];
}

export function mergeProtonDbEnvVarGroup(
  currentEnvVars: Record<string, string>,
  group: ProtonDbRecommendationGroup,
  overwriteKeys: readonly string[] = []
): ProtonDbEnvVarMergeResult {
  const overwriteSet = new Set(overwriteKeys);
  const mergedEnvVars = { ...currentEnvVars };
  const conflicts: ProtonDbEnvVarConflict[] = [];
  const appliedKeys: string[] = [];
  const unchangedKeys: string[] = [];

  const envVars = group.env_vars;
  if (!envVars || envVars.length === 0) {
    return {
      mergedEnvVars,
      conflicts,
      appliedKeys,
      unchangedKeys,
    };
  }

  for (const envVar of envVars) {
    const existingValue = currentEnvVars[envVar.key];

    if (existingValue === undefined) {
      mergedEnvVars[envVar.key] = envVar.value;
      appliedKeys.push(envVar.key);
      continue;
    }

    if (existingValue === envVar.value) {
      unchangedKeys.push(envVar.key);
      continue;
    }

    if (!overwriteSet.has(envVar.key)) {
      conflicts.push({
        key: envVar.key,
        currentValue: existingValue,
        suggestedValue: envVar.value,
      });
      continue;
    }

    mergedEnvVars[envVar.key] = envVar.value;
    appliedKeys.push(envVar.key);
  }

  return {
    mergedEnvVars,
    conflicts,
    appliedKeys,
    unchangedKeys,
  };
}

export interface ProtonDbApplyResult {
  nextProfile: GameProfile;
  appliedKeys: string[];
  unchangedKeys: string[];
  toggledOptionIds: string[];
}

export function applyProtonDbGroupToProfile(
  current: GameProfile,
  group: ProtonDbRecommendationGroup,
  overwriteKeys: readonly string[],
  catalog: OptimizationCatalogPayload | null
): ProtonDbApplyResult {
  const nextMerge = mergeProtonDbEnvVarGroup(current.launch.custom_env_vars, group, overwriteKeys);

  // Build catalog env index: "key=value" -> entry id
  const catalogIndex = new Map<string, string>();
  if (catalog?.entries) {
    for (const entry of catalog.entries) {
      for (const [k, v] of entry.env) {
        catalogIndex.set(`${k}=${v}`, entry.id);
      }
    }
  }

  // Route applied keys through catalog matching
  const customEnvVars = { ...nextMerge.mergedEnvVars };
  const enabledOptionIds = [...(current.launch.optimizations?.enabled_option_ids ?? [])];
  const toggledOptionIds: string[] = [];

  for (const key of nextMerge.appliedKeys) {
    const value = customEnvVars[key];
    if (value === undefined) continue;
    const entryId = catalogIndex.get(`${key}=${value}`);
    if (entryId) {
      delete customEnvVars[key]; // Catalog-matched: always remove from custom_env_vars
      if (!enabledOptionIds.includes(entryId)) {
        enabledOptionIds.push(entryId);
        toggledOptionIds.push(entryId);
      }
    }
  }

  const nextProfile: GameProfile = {
    ...current,
    launch: {
      ...current.launch,
      custom_env_vars: customEnvVars,
      optimizations: {
        ...current.launch.optimizations,
        enabled_option_ids: enabledOptionIds,
      },
    },
  };

  return {
    nextProfile,
    appliedKeys: nextMerge.appliedKeys,
    unchangedKeys: nextMerge.unchangedKeys,
    toggledOptionIds,
  };
}
