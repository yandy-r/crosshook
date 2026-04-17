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
