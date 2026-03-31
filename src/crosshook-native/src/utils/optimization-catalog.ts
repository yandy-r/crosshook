import { invoke } from '@tauri-apps/api/core';

/** A single optimization entry from the Rust catalog, including both
 *  functional fields (env, wrappers) and UI metadata (label, description). */
export interface OptimizationEntry {
  id: string;
  applies_to_method: string;
  env: [string, string][];
  wrappers: string[];
  conflicts_with: string[];
  required_binary: string;
  label: string;
  description: string;
  help_text: string;
  category: string;
  target_gpu_vendor: string;
  advanced: boolean;
  community: boolean;
  applicable_methods: string[];
}

/** The full optimization catalog payload returned by the Tauri IPC command. */
export interface OptimizationCatalogPayload {
  catalog_version: number;
  entries: OptimizationEntry[];
}

let _cached: OptimizationCatalogPayload | null = null;

/** Fetches the optimization catalog from the Rust backend.
 *  Caches the result in memory — subsequent calls return instantly. */
export async function fetchOptimizationCatalog(): Promise<OptimizationCatalogPayload> {
  if (_cached) return _cached;
  _cached = await invoke<OptimizationCatalogPayload>('get_optimization_catalog');
  return _cached;
}

/** Returns the cached catalog, or null if not yet fetched. */
export function getCachedCatalog(): OptimizationCatalogPayload | null {
  return _cached;
}

/** Build a lookup map from entry id to entry. */
export function buildOptionsById(
  entries: readonly OptimizationEntry[]
): Record<string, OptimizationEntry> {
  const map: Record<string, OptimizationEntry> = {};
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
export function buildConflictMatrix(
  entries: readonly OptimizationEntry[]
): Record<string, readonly string[]> {
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
