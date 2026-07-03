// Pure helpers for the mod coexistence registry UI.

import type { DetectedModCandidate, ModCategory, ProfileModInput, ProfileModRecord } from '@/types/mods';

export const MOD_CATEGORY_LABELS: Record<ModCategory, string> = {
  overlay_injection: 'Overlay / injection',
  script_extender: 'Script extender',
  file_replacement: 'File replacement',
  other: 'Other',
};

export const MOD_CATEGORY_OPTIONS: { value: ModCategory; label: string }[] = (
  Object.keys(MOD_CATEGORY_LABELS) as ModCategory[]
).map((value) => ({ value, label: MOD_CATEGORY_LABELS[value] }));

const CATEGORY_ORDER: Record<ModCategory, number> = {
  overlay_injection: 0,
  script_extender: 1,
  file_replacement: 2,
  other: 3,
};

export function isHttpUrl(value: string): boolean {
  return /^https?:\/\//i.test(value.trim());
}

export function summarizeModPaths(paths: string[]): { summary: string; full: string } {
  if (paths.length === 0) {
    return { summary: '', full: '' };
  }
  const first = paths[0];
  const basename = first.split(/[\\/]/).filter(Boolean).pop() ?? first;
  const summary = paths.length > 1 ? `${basename} +${paths.length - 1} more` : basename;
  return { summary, full: paths.join('\n') };
}

/** Enabled first → category (label order) → name (case-insensitive). */
export function sortMods(mods: ProfileModRecord[]): ProfileModRecord[] {
  return [...mods].sort((a, b) => {
    if (a.enabled !== b.enabled) {
      return a.enabled ? -1 : 1;
    }
    if (CATEGORY_ORDER[a.category] !== CATEGORY_ORDER[b.category]) {
      return CATEGORY_ORDER[a.category] - CATEGORY_ORDER[b.category];
    }
    return a.name.toLowerCase().localeCompare(b.name.toLowerCase());
  });
}

export function candidateToInput(candidate: DetectedModCandidate): ProfileModInput {
  return {
    name: candidate.suggested_name,
    category: candidate.category,
    paths: candidate.matched_paths,
    enabled: true,
    source_url: undefined,
    provenance: 'detected',
  };
}
