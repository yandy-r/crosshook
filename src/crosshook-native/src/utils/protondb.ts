import type { ProtonDbRecommendationGroup } from '../types/protondb';

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
