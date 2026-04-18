import type { LaunchOptimizationId } from '../../types/launch-optimizations';
import type { OptimizationEntry } from '../../utils/optimization-catalog';

export function normalizeLaunchOptimizationIds(
  ids: readonly string[] | undefined,
  optionsById: Record<string, OptimizationEntry>,
  catalogLoaded: boolean
): LaunchOptimizationId[] {
  if (ids === undefined) {
    return [];
  }

  /** When false, catalog fetch has not completed — pass IDs through (lenient). When true, filter to known IDs (strict, including empty catalog). */
  const catalogReadyForFiltering = catalogLoaded;
  const normalized: LaunchOptimizationId[] = [];
  const seenIds = new Set<LaunchOptimizationId>();

  for (const optionId of ids) {
    // When catalog is not yet loaded, pass IDs through without filtering (lenient mode).
    if (catalogReadyForFiltering && !(optionId in optionsById)) {
      continue;
    }

    const typedOptionId = optionId as LaunchOptimizationId;
    if (seenIds.has(typedOptionId)) {
      continue;
    }

    seenIds.add(typedOptionId);
    normalized.push(typedOptionId);
  }

  return normalized;
}

export function areLaunchOptimizationIdsEqual(
  left: readonly LaunchOptimizationId[],
  right: readonly LaunchOptimizationId[]
): boolean {
  if (left.length !== right.length) {
    return false;
  }

  return left.every((optionId, index) => optionId === right[index]);
}
