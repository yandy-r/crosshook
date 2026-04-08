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
    if (collections.some((c) => c.name === trimmed)) {
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
    const { collection_id } = args as { collection_id: string };
    collections = collections.filter((c) => c.collection_id !== collection_id);
    membership.delete(collection_id);
    return null;
  });

  map.set('collection_add_profile', async (args): Promise<null> => {
    const { collection_id, profile_name } = args as {
      collection_id: string;
      profile_name: string;
    };
    if (!findById(collection_id)) {
      throw new Error(
        `[dev-mock] collection_add_profile: collection not found: ${collection_id}`
      );
    }
    const trimmed = (profile_name ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] collection_add_profile: profile_name must not be empty');
    }
    const store = getStore();
    if (!store.profiles.has(trimmed)) {
      throw new Error(
        `[dev-mock] collection_add_profile: profile not found: ${trimmed}`
      );
    }
    const set = membership.get(collection_id) ?? new Set<string>();
    set.add(trimmed);
    membership.set(collection_id, set);
    return null;
  });

  map.set('collection_remove_profile', async (args): Promise<null> => {
    const { collection_id, profile_name } = args as {
      collection_id: string;
      profile_name: string;
    };
    const trimmed = (profile_name ?? '').trim();
    // Idempotent — matches Rust semantics at collections.rs:117-120.
    membership.get(collection_id)?.delete(trimmed);
    return null;
  });

  map.set('collection_list_profiles', async (args): Promise<string[]> => {
    const { collection_id } = args as { collection_id: string };
    const set = membership.get(collection_id);
    return set ? [...set].sort() : [];
  });

  map.set('collection_rename', async (args): Promise<null> => {
    const { collection_id, new_name } = args as {
      collection_id: string;
      new_name: string;
    };
    const trimmed = (new_name ?? '').trim();
    if (!trimmed) {
      throw new Error('[dev-mock] collection_rename: collection name must not be empty');
    }
    const target = findById(collection_id);
    if (!target) {
      throw new Error(`[dev-mock] collection_rename: collection not found: ${collection_id}`);
    }
    if (collections.some((c) => c.collection_id !== collection_id && c.name === trimmed)) {
      throw new Error(`[dev-mock] collection_rename: duplicate collection name: ${trimmed}`);
    }
    target.name = trimmed;
    target.updated_at = nowIso();
    return null;
  });

  map.set('collection_update_description', async (args): Promise<null> => {
    const { collection_id, description } = args as {
      collection_id: string;
      description: string | null;
    };
    const target = findById(collection_id);
    if (!target) {
      throw new Error(
        `[dev-mock] collection_update_description: collection not found: ${collection_id}`
      );
    }
    const normalized = description?.trim();
    target.description = normalized ? normalized : null;
    target.updated_at = nowIso();
    return null;
  });

  map.set('collections_for_profile', async (args): Promise<MockCollectionRow[]> => {
    const { profile_name } = args as { profile_name: string };
    const trimmed = (profile_name ?? '').trim();
    recomputeProfileCounts();
    return collections
      .filter((c) => membership.get(c.collection_id)?.has(trimmed))
      .sort((a, b) => a.name.localeCompare(b.name));
  });
}
