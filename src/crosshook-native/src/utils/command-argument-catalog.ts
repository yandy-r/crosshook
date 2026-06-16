import { callCommand } from '@/lib/ipc';
import type { CommandArgumentCatalogPayload, CommandArgumentEntry } from '@/types/launch-command-arguments';

let _cached: CommandArgumentCatalogPayload | null = null;

/** Fetches the command-argument catalog from the Rust backend.
 *  Caches the result in memory — subsequent calls return instantly. */
export async function fetchCommandArgumentCatalog(): Promise<CommandArgumentCatalogPayload> {
  if (_cached) return _cached;
  _cached = await callCommand<CommandArgumentCatalogPayload>('get_command_argument_catalog');
  return _cached;
}

/** Returns the cached catalog, or null if not yet fetched. */
export function getCachedCommandArgumentCatalog(): CommandArgumentCatalogPayload | null {
  return _cached;
}

/** Build a lookup map from entry id to entry. */
export function buildArgumentsById(entries: readonly CommandArgumentEntry[]): Record<string, CommandArgumentEntry> {
  const map: Record<string, CommandArgumentEntry> = {};
  for (const entry of entries) {
    map[entry.id] = entry;
  }
  return map;
}

/** Build a conflict matrix: for each entry id, the list of ids it conflicts with.
 *
 * Declared `conflicts_with` edges are normalized to be bidirectional so the UI
 * and toggle logic stay consistent when the catalog lists a conflict on only one side.
 */
export function buildConflictMatrix(entries: readonly CommandArgumentEntry[]): Record<string, readonly string[]> {
  const knownIds = new Set(entries.map((e) => e.id));
  const mutable: Record<string, Set<string>> = {};
  for (const id of knownIds) {
    mutable[id] = new Set();
  }

  for (const entry of entries) {
    const from = entry.id;
    for (const to of entry.conflicts_with ?? []) {
      if (to === from) {
        continue;
      }
      mutable[from]?.add(to);
      if (knownIds.has(to)) {
        mutable[to]?.add(from);
      }
    }
  }

  const matrix: Record<string, readonly string[]> = {};
  for (const id of knownIds) {
    const list = Array.from(mutable[id] ?? []).sort();
    matrix[id] = Object.freeze(list);
  }
  return matrix;
}
