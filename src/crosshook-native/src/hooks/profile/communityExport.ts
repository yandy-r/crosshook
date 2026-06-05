export function suggestedCommunityExportFilename(profileName: string): string {
  const base = profileName
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `${base || 'community-profile'}.json`;
}
