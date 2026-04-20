import type { ConfigRevisionSource } from '../../types/profile-history';

/** Human-readable labels for revision sources. */
export const SOURCE_LABELS: Record<ConfigRevisionSource, string> = {
  manual_save: 'Manual save',
  rollback_apply: 'Restore',
  import: 'Import',
  launch_optimization_save: 'Optimization save',
  preset_apply: 'Preset applied',
  migration: 'Migration',
};

/**
 * Format an ISO date string into a locale-specific readable format.
 * Falls back to the original string if parsing fails.
 */
export function formatExactDate(isoString: string): string {
  try {
    return new Date(isoString).toLocaleString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return isoString;
  }
}
