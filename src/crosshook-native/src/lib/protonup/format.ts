/**
 * Formatting helpers for the Proton manager row metadata caption.
 *
 * Kept co-located with `classifyInstall.ts` to minimize import sprawl. If we
 * later need a bytes formatter elsewhere in the UI, promote it to a shared
 * `lib/format.ts`.
 */

export function formatBytes(value: number | null | undefined): string | null {
  if (value == null || !Number.isFinite(value) || value <= 0) return null;
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let current = value;
  let unitIndex = 0;
  while (current >= 1024 && unitIndex < units.length - 1) {
    current /= 1024;
    unitIndex += 1;
  }
  const decimals = current >= 100 ? 0 : current >= 10 ? 1 : 2;
  return `${current.toFixed(decimals)} ${units[unitIndex]}`;
}

/** Format an ISO-8601 timestamp as a short local date, e.g. "Apr 10, 2025". */
export function formatReleaseDate(iso: string | null | undefined): string | null {
  if (!iso) return null;
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return null;
  return date.toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  });
}
