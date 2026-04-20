import { LAUNCH_OPTIMIZATION_CATEGORIES, type LaunchOptimizationCategory } from '../../types/launch-optimizations';
import type { OptimizationEntry } from '../../utils/optimization-catalog';

export type CapabilityId = 'gamescope' | 'mangohud' | 'gamemode' | 'prefix_tools' | 'non_steam_launch';

export function joinClasses(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(' ');
}

export function formatCountLabel(count: number, singular: string, plural: string): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

export interface GroupedOptions {
  category: LaunchOptimizationCategory;
  options: OptimizationEntry[];
}

export function groupOptions(options: readonly OptimizationEntry[]): GroupedOptions[] {
  return LAUNCH_OPTIMIZATION_CATEGORIES.map((category) => ({
    category,
    options: options.filter((option) => option.category === category),
  })).filter((group) => group.options.length > 0);
}
