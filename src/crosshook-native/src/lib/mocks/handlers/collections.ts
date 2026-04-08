// Mock IPC handlers for collection_* commands. See `lib/mocks/README.md`.
// All error messages MUST start with `[dev-mock]` to participate in the
// `.github/workflows/release.yml` "Verify no mock code in production bundle"
// sentinel.

import type { Handler } from './types';
import { getStore } from '../store';

// Shape mirrors Rust `CollectionRow` in
// crates/crosshook-core/src/metadata/models.rs (snake_case per serde default).
interface MockCollectionRow {
  collection_id: string;
  name: string;
  description: string | null;
  profile_count: number;
  created_at: string;
  updated_at: string;
}

// Module-scope mutable state — resets on page reload.
let collections: MockCollectionRow[] = [
  {
    collection_id: 'mock-collection-1',
    name: 'Action / Adventure',
    description: 'Seeded fixture collection for dev mode',
    profile_count: 0,
    created_at: new Date('2026-04-01T12:00:00Z').toISOString(),
    updated_at: new Date('2026-04-01T12:00:00Z').toISOString(),
  },
];
const membership = new Map<string, Set<string>>([['mock-collection-1', new Set()]]);

// Shape mirrors Rust `CollectionDefaultsSection` in
// crates/crosshook-core/src/profile/models.rs. All fields optional;
// `custom_env_vars` is an additive merge bucket. Inner shapes for gamescope/
// trainer_gamescope/mangohud are intentionally `unknown` because the mock layer
// does not need to introspect them — Rust performs the canonical merge in
// production. The browser dev-mode merge in profile.ts performs a structural
// replacement that mirrors the Rust semantics for the editable subset.
export interface MockCollectionDefaults {
  method?: string;
  optimizations?: { enabled_option_ids: string[] };
  custom_env_vars?: Record<string, string>;
  network_isolation?: boolean;
  gamescope?: unknown;
  trainer_gamescope?: unknown;
  mangohud?: unknown;
}

const mockDefaults = new Map<string, MockCollectionDefaults>();

function isDefaultsEmpty(d: MockCollectionDefaults | undefined | null): boolean {
  if (!d) return true;
  return (
    d.method === undefined &&
    d.optimizations === undefined &&
    (d.custom_env_vars === undefined || Object.keys(d.custom_env_vars).length === 0) &&
    d.network_isolation === undefined &&
    d.gamescope === undefined &&
    d.trainer_gamescope === undefined &&
    d.mangohud === undefined
  );
}

/** Used by the profile_load mock to apply collection defaults to a loaded profile. */
export function getMockCollectionDefaults(
  collectionId: string
): MockCollectionDefaults | undefined {
  return mockDefaults.get(collectionId);
}

function nowIso(): string {
  return new Date().toISOString();
}

function recomputeProfileCounts(): void {
  for (const col of collections) {
    col.profile_count = membership.get(col.collection_id)?.size ?? 0;
  }
}

function findById(id: string): MockCollectionRow | undefined {
  return collections.find((c) => c.collection_id === id);
}

/** Match SQLite `COLLATE NOCASE` / typical backend duplicate checks (ASCII case-insensitive). */
function nameCollidesWithExisting(name: string): boolean {
  const lower = name.toLowerCase();
  return collections.some((c) => c.name.toLowerCase() === lower);
}

export function registerCollections(map: Map<string, Handler>): void {
  map.set('collection_list', async (): Promise<MockCollectionRow[]> => {
    recomputeProfileCounts();
    // Mirror Rust ORDER BY c.sort_order ASC, c.name ASC — sort_order is not
    // tracked in the mock; fall back to name ordering.
    return [...collections].sort((a, b) => a.name.localeCompare(b.name));
  });

  map.set('collection_create', async (args): Promise<string> => {
    const { name } = args as { name: string };
    const trimmed = (name ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] collection_create: collection name must not be empty');
    }
    if (nameCollidesWithExisting(trimmed)) {
      throw new Error(`[dev-mock] collection_create: duplicate collection name: ${trimmed}`);
    }
    const id = `mock-collection-${Date.now().toString(36)}`;
    const ts = nowIso();
    collections = [
      ...collections,
      {
        collection_id: id,
        name: trimmed,
        description: null,
        profile_count: 0,
        created_at: ts,
        updated_at: ts,
      },
    ];
    membership.set(id, new Set());
    return id;
  });

  map.set('collection_delete', async (args): Promise<null> => {
    const { collectionId } = args as { collectionId: string };
    collections = collections.filter((c) => c.collection_id !== collectionId);
    membership.delete(collectionId);
    return null;
  });

  map.set('collection_add_profile', async (args): Promise<null> => {
    const { collectionId, profileName } = args as {
      collectionId: string;
      profileName: string;
    };
    if (!findById(collectionId)) {
      throw new Error(
        `[dev-mock] collection_add_profile: collection not found: ${collectionId}`
      );
    }
    const trimmed = (profileName ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] collection_add_profile: profileName must not be empty');
    }
    const store = getStore();
    if (!store.profiles.has(trimmed)) {
      throw new Error(
        `[dev-mock] collection_add_profile: profile not found: ${trimmed}`
      );
    }
    const set = membership.get(collectionId) ?? new Set<string>();
    set.add(trimmed);
    membership.set(collectionId, set);
    return null;
  });

  map.set('collection_remove_profile', async (args): Promise<null> => {
    const { collectionId, profileName } = args as {
      collectionId: string;
      profileName: string;
    };
    const trimmed = (profileName ?? '').trim();
    // Idempotent — matches Rust semantics at collections.rs:117-120.
    membership.get(collectionId)?.delete(trimmed);
    return null;
  });

  map.set('collection_list_profiles', async (args): Promise<string[]> => {
    const { collectionId } = args as { collectionId: string };
    const set = membership.get(collectionId);
    return set ? [...set].sort() : [];
  });

  map.set('collection_rename', async (args): Promise<null> => {
    const { collectionId, newName } = args as {
      collectionId: string;
      newName: string;
    };
    const trimmed = (newName ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] collection_rename: collection name must not be empty');
    }
    const target = findById(collectionId);
    if (!target) {
      throw new Error(`[dev-mock] collection_rename: collection not found: ${collectionId}`);
    }
    if (
      collections.some(
        (c) => c.collection_id !== collectionId && c.name.toLowerCase() === trimmed.toLowerCase()
      )
    ) {
      throw new Error(`[dev-mock] collection_rename: duplicate collection name: ${trimmed}`);
    }
    target.name = trimmed;
    target.updated_at = nowIso();
    return null;
  });

  map.set('collection_update_description', async (args): Promise<null> => {
    const { collectionId, description } = args as {
      collectionId: string;
      description: string | null;
    };
    const target = findById(collectionId);
    if (!target) {
      throw new Error(
        `[dev-mock] collection_update_description: collection not found: ${collectionId}`
      );
    }
    const normalized = description?.trim();
    target.description = normalized ? normalized : null;
    target.updated_at = nowIso();
    return null;
  });

  map.set('collections_for_profile', async (args): Promise<MockCollectionRow[]> => {
    const { profileName } = args as { profileName: string };
    const trimmed = (profileName ?? '').trim();
    recomputeProfileCounts();
    return collections
      .filter((c) => membership.get(c.collection_id)?.has(trimmed))
      .sort((a, b) => a.name.localeCompare(b.name));
  });

  map.set(
    'collection_get_defaults',
    async (args): Promise<MockCollectionDefaults | null> => {
      const { collectionId } = args as { collectionId: string };
      if (!findById(collectionId)) {
        throw new Error(
          `[dev-mock] collection_get_defaults: collection not found: ${collectionId}`
        );
      }
      const d = mockDefaults.get(collectionId);
      return d && !isDefaultsEmpty(d) ? d : null;
    }
  );

  map.set('collection_set_defaults', async (args): Promise<null> => {
    const { collectionId, defaults } = args as {
      collectionId: string;
      defaults: MockCollectionDefaults | null;
    };
    const target = findById(collectionId);
    if (!target) {
      throw new Error(
        `[dev-mock] collection_set_defaults: collection not found: ${collectionId}`
      );
    }
    if (defaults === null || isDefaultsEmpty(defaults)) {
      mockDefaults.delete(collectionId);
    } else {
      mockDefaults.set(collectionId, { ...defaults });
    }
    target.updated_at = nowIso();
    return null;
  });
}
