import type { ProtonInstallOption } from '../types/proton';

export function formatProtonInstallLabel(
  install: ProtonInstallOption,
  duplicateNameCounts: Record<string, number>
): string {
  const baseLabel = install.name.trim() || 'Unnamed Proton install';
  if ((duplicateNameCounts[baseLabel] ?? 0) <= 1) {
    return baseLabel;
  }

  return `${baseLabel} (${install.is_official ? 'Steam' : 'Custom'})`;
}
