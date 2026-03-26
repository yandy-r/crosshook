export function deriveSteamClientInstallPath(compatdataPath: string): string {
  const marker = '/steamapps/compatdata/';
  const normalized = compatdataPath.trim().replace(/\\/g, '/');
  const index = normalized.indexOf(marker);

  return index >= 0 ? normalized.slice(0, index) : '';
}

export function deriveTargetHomePath(steamClientInstallPath: string): string {
  const normalized = steamClientInstallPath.trim().replace(/\\/g, '/');

  for (const suffix of ['/.local/share/Steam', '/.steam/root']) {
    if (normalized.endsWith(suffix)) {
      return normalized.slice(0, -suffix.length);
    }
  }

  return '';
}
