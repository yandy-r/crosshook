/**
 * Deterministic, order-independent signature for a custom env var record.
 *
 * Sorts entries by key so two records with the same key/value pairs in a
 * different insertion order produce identical signatures. Used for change
 * detection / dedupe (e.g. autosave debouncing, dirty-row diffing) — never
 * for persistence or serialization of the record itself.
 */
export function envVarSignature(record: Readonly<Record<string, string>>): string {
  const sortedEntries = Object.entries(record).sort(([a], [b]) => a.localeCompare(b));
  return JSON.stringify(sortedEntries);
}
