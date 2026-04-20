import type { MigrationSuggestion } from '../../types';

export const FIELD_LABELS: Record<string, string> = {
  steam_proton_path: 'Steam Proton',
  runtime_proton_path: 'Runtime Proton',
};

export function rowKey(s: MigrationSuggestion): string {
  return `${s.profile_name}:${s.field}`;
}

export function isSafe(s: MigrationSuggestion): boolean {
  return !s.crosses_major_version && s.confidence >= 0.75;
}

export interface ConfidenceInfo {
  text: string;
  color: string;
}

export function getConfidenceInfo(s: MigrationSuggestion): ConfidenceInfo {
  if (s.confidence < 0.75) {
    return { text: 'Different family', color: 'var(--crosshook-color-warning)' };
  }
  if (s.crosses_major_version) {
    return { text: 'Major version change', color: 'var(--crosshook-color-warning)' };
  }
  if (s.confidence >= 0.9) {
    return { text: 'Upgrade', color: 'var(--crosshook-color-success)' };
  }
  return { text: 'Older version', color: 'var(--crosshook-color-warning)' };
}
