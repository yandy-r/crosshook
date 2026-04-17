/**
 * Classify an installed Proton tool by its directory name, returning the
 * stable provider id it most likely belongs to.
 *
 * Installed inventory is rescan-derived runtime state (per AGENTS.md), so
 * this mapping is pure and not persisted. Add a new entry when a new
 * provider is registered in `crosshook-core::protonup::providers`.
 *
 * Returns `null` for unknown names; the UI treats those as "ungrouped" and
 * only shows them under the "All" filter.
 */
const PROVIDER_PREFIXES: ReadonlyArray<readonly [RegExp, string]> = [
  [/^GE-Proton/i, 'ge-proton'],
  [/^proton-?cachyos/i, 'proton-cachyos'],
  [/^Proton-EM/i, 'proton-em'],
  [/^Luxtorpeda/i, 'luxtorpeda'],
  [/^Boxtron/i, 'boxtron'],
];

export function classifyInstallProvider(name: string): string | null {
  for (const [re, id] of PROVIDER_PREFIXES) {
    if (re.test(name)) return id;
  }
  return null;
}

/**
 * Normalize an installed directory name to a key comparable with the
 * provider's GitHub release tag.
 *
 * Proton-CachyOS publishes releases with tags like `cachyos-10.0-20260410-slr`
 * but ships archives whose extracted directory name is
 * `proton-cachyos-10.0-20260410-slr-x86_64`. Exact-string matching would
 * perpetually mark CachyOS installs as "Available"; this helper strips the
 * `proton-` prefix and the `-x86_64[_v3]` arch suffix so the key aligns with
 * the tag.
 *
 * GE-Proton and Proton-EM directory names already equal their tags.
 */
export function normalizeInstallToTag(installName: string, providerId: string | null): string {
  if (providerId === 'proton-cachyos') {
    // `proton-cachyos-<tag>-x86_64` → `<tag>` (strip `proton-` prefix and arch suffix).
    return installName.replace(/^proton-/, '').replace(/-x86_64(?:_v3)?$/, '');
  }
  if (providerId === 'proton-em') {
    // `proton-<tag>` (e.g. `proton-EM-10.0-37-HDR`) → `<tag>` (e.g. `EM-10.0-37-HDR`).
    return installName.replace(/^proton-/, '');
  }
  return installName;
}
