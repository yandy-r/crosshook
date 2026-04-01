import type { ProtonDbRecommendationGroup } from '../types/protondb';

export interface ProtonDbEnvVarConflict {
  key: string;
  currentValue: string;
  suggestedValue: string;
}

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

  for (const envVar of group.env_vars) {
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
