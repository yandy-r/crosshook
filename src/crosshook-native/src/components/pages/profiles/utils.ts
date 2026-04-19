import type { ProtonInstallOption } from '../../../types/proton';

export function suggestedCommunityExportFilename(profileName: string): string {
  const base = profileName
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `${base || 'community-profile'}.json`;
}

export function sortProtonInstalls(installs: ProtonInstallOption[]): ProtonInstallOption[] {
  return [...installs].sort((left, right) => {
    if (left.is_official !== right.is_official) {
      return left.is_official ? -1 : 1;
    }

    return left.name.localeCompare(right.name) || left.path.localeCompare(right.path);
  });
}
