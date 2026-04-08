import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from 'react';

import { callCommand } from '@/lib/ipc';
import type {
  CollectionImportPreview,
  CollectionExportResult,
  CollectionRow,
} from '@/types/collections';
import { isCollectionDefaultsEmpty } from '@/types/profile';
import { chooseSaveFile } from '@/utils/dialog';

/** Result of rename / description updates so callers can show session-scoped errors without reading global hook state. */
export type CollectionWriteResult = { ok: true } | { ok: false; error: string };

/** Create flow: collection row may succeed while description IPC fails; `descriptionFailed` is set in that case (refresh still runs). */
export type CollectionCreateResult =
  | { ok: true; id: string; descriptionFailed?: string }
  | { ok: false; error: string };

export type CollectionMutationResult = { ok: true } | { ok: false; error: string };

export type CollectionImportApplyResult = { ok: true } | { ok: false; error: string };

export interface ApplyImportedCollectionInput {
  preview: CollectionImportPreview;
  name: string;
  description: string | null;
  /** One entry per `preview.ambiguous` row: chosen local profile name, or `null` to skip. */
  ambiguousResolutions: (string | null)[];
}

export interface UseCollectionsResult {
  collections: CollectionRow[];
  error: string | null;
  isListing: boolean;
  creatingName: string | null;
  deletingId: string | null;
  renamingId: string | null;
  refresh: () => Promise<void>;
  createCollection: (name: string, description?: string | null) => Promise<CollectionCreateResult>;
  deleteCollection: (collectionId: string) => Promise<boolean>;
  renameCollection: (collectionId: string, newName: string) => Promise<CollectionWriteResult>;
  updateDescription: (collectionId: string, description: string | null) => Promise<CollectionWriteResult>;
  addProfile: (collectionId: string, profileName: string) => Promise<CollectionMutationResult>;
  removeProfile: (collectionId: string, profileName: string) => Promise<CollectionMutationResult>;
  listMembers: (collectionId: string) => Promise<string[]>;
  collectionsForProfile: (profileName: string) => Promise<CollectionRow[]>;
  /** Phase 4: preview-only parse for a preset file path (no SQLite writes). */
  prepareCollectionImportPreview: (path: string) => Promise<CollectionImportPreview>;
  /** Phase 4: create collection + defaults + memberships from a preview; one `refresh()` on success. */
  applyImportedCollection: (input: ApplyImportedCollectionInput) => Promise<CollectionImportApplyResult>;
  /** Phase 4: save preset for the current collection; returns `'cancelled'` if the save dialog was dismissed. */
  exportCollectionPreset: (
    collectionId: string,
    options?: { /** Browser dev: skip native save dialog and use this path for mock IPC */ outputPathOverride?: string }
  ) => Promise<'cancelled' | { ok: true } | { ok: false; error: string }>;
}

const CollectionsContext = createContext<UseCollectionsResult | null>(null);

function useCollectionsState(): UseCollectionsResult {
  const [collections, setCollections] = useState<CollectionRow[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [isListing, setIsListing] = useState(false);
  const [creatingName, setCreatingName] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setIsListing(true);
    try {
      const result = await callCommand<CollectionRow[]>('collection_list');
      setCollections(result);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsListing(false);
    }
  }, []);

  const createCollection = useCallback(
    async (name: string, description: string | null = null): Promise<CollectionCreateResult> => {
      setCreatingName(name);
      setError(null);
      try {
        const id = await callCommand<string>('collection_create', { name });
        let descriptionFailed: string | undefined;
        if (description !== null && description.trim() !== '') {
          try {
            await callCommand<null>('collection_update_description', {
              collectionId: id,
              description: description.trim(),
            });
          } catch (err) {
            descriptionFailed = err instanceof Error ? err.message : String(err);
          }
        }
        await refresh();
        return descriptionFailed !== undefined
          ? { ok: true, id, descriptionFailed }
          : { ok: true, id };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        return { ok: false, error: message };
      } finally {
        setCreatingName(null);
      }
    },
    [refresh]
  );

  const deleteCollection = useCallback(
    async (collectionId: string): Promise<boolean> => {
      setDeletingId(collectionId);
      setError(null);
      try {
        await callCommand<null>('collection_delete', { collectionId });
        await refresh();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setDeletingId(null);
      }
    },
    [refresh]
  );

  const renameCollection = useCallback(
    async (collectionId: string, newName: string): Promise<CollectionWriteResult> => {
      setRenamingId(collectionId);
      setError(null);
      try {
        await callCommand<null>('collection_rename', { collectionId, newName });
        await refresh();
        return { ok: true };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        return { ok: false, error: message };
      } finally {
        setRenamingId(null);
      }
    },
    [refresh]
  );

  const updateDescription = useCallback(
    async (collectionId: string, description: string | null): Promise<CollectionWriteResult> => {
      setError(null);
      try {
        await callCommand<null>('collection_update_description', { collectionId, description });
        await refresh();
        return { ok: true };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        return { ok: false, error: message };
      }
    },
    [refresh]
  );

  const addProfile = useCallback(
    async (collectionId: string, profileName: string): Promise<CollectionMutationResult> => {
      setError(null);
      try {
        await callCommand<null>('collection_add_profile', { collectionId, profileName });
        await refresh();
        return { ok: true };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        return { ok: false, error: message };
      }
    },
    [refresh]
  );

  const removeProfile = useCallback(
    async (collectionId: string, profileName: string): Promise<CollectionMutationResult> => {
      setError(null);
      try {
        await callCommand<null>('collection_remove_profile', { collectionId, profileName });
        await refresh();
        return { ok: true };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        return { ok: false, error: message };
      }
    },
    [refresh]
  );

  const listMembers = useCallback(async (collectionId: string): Promise<string[]> => {
    try {
      return await callCommand<string[]>('collection_list_profiles', { collectionId });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return [];
    }
  }, []);

  const collectionsForProfile = useCallback(async (profileName: string): Promise<CollectionRow[]> => {
    try {
      return await callCommand<CollectionRow[]>('collections_for_profile', { profileName });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return [];
    }
  }, []);

  const prepareCollectionImportPreview = useCallback(async (path: string): Promise<CollectionImportPreview> => {
    setError(null);
    return await callCommand<CollectionImportPreview>('collection_import_from_toml', { path });
  }, []);

  const applyImportedCollection = useCallback(
    async (input: ApplyImportedCollectionInput): Promise<CollectionImportApplyResult> => {
      const { preview, name, description, ambiguousResolutions } = input;
      const trimmed = name.trim();
      if (trimmed === '') {
        return { ok: false, error: 'Collection name must not be empty' };
      }

      const duplicate = collections.some((c) => c.name.toLowerCase() === trimmed.toLowerCase());
      if (duplicate) {
        return { ok: false, error: `A collection named "${trimmed}" already exists.` };
      }

      if (ambiguousResolutions.length !== preview.ambiguous.length) {
        return { ok: false, error: 'Ambiguous resolution count does not match preview.' };
      }

      let createdId: string | null = null;
      try {
        createdId = await callCommand<string>('collection_create', { name: trimmed });
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        return { ok: false, error: message };
      }

      try {
        if (description !== null && description.trim() !== '') {
          await callCommand('collection_update_description', {
            collectionId: createdId,
            description: description.trim(),
          });
        }

        const defaults = preview.manifest.defaults;
        if (defaults !== undefined && defaults !== null && !isCollectionDefaultsEmpty(defaults)) {
          await callCommand('collection_set_defaults', { collectionId: createdId, defaults });
        }

        for (const m of preview.matched) {
          await callCommand('collection_add_profile', {
            collectionId: createdId,
            profileName: m.local_profile_name,
          });
        }

        for (let i = 0; i < preview.ambiguous.length; i++) {
          const pick = ambiguousResolutions[i];
          if (pick !== null && pick.trim() !== '') {
            await callCommand('collection_add_profile', {
              collectionId: createdId,
              profileName: pick.trim(),
            });
          }
        }

        await refresh();
        return { ok: true };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        if (createdId !== null) {
          try {
            await callCommand('collection_delete', { collectionId: createdId });
          } catch (rollbackErr) {
            console.error('collection import rollback failed', rollbackErr);
          }
        }
        await refresh();
        return { ok: false, error: message };
      }
    },
    [collections, refresh]
  );

  const exportCollectionPreset = useCallback(
    async (
      collectionId: string,
      options?: { outputPathOverride?: string }
    ): Promise<'cancelled' | { ok: true } | { ok: false; error: string }> => {
      const path =
        options?.outputPathOverride ??
        (await chooseSaveFile('Export collection preset', {
          defaultPath: 'collection.crosshook-collection.toml',
          filters: [{ name: 'CrossHook collection preset', extensions: ['toml'] }],
        }));
      if (path === null) {
        return 'cancelled';
      }
      setError(null);
      try {
        await callCommand<CollectionExportResult>('collection_export_to_toml', {
          collectionId,
          outputPath: path,
        });
        return { ok: true };
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        setError(message);
        return { ok: false, error: message };
      }
    },
    []
  );

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return {
    collections,
    error,
    isListing,
    creatingName,
    deletingId,
    renamingId,
    refresh,
    createCollection,
    deleteCollection,
    renameCollection,
    updateDescription,
    addProfile,
    removeProfile,
    listMembers,
    collectionsForProfile,
    prepareCollectionImportPreview,
    applyImportedCollection,
    exportCollectionPreset,
  };
}

export function CollectionsProvider({ children }: { children: ReactNode }) {
  const value = useCollectionsState();
  return <CollectionsContext.Provider value={value}>{children}</CollectionsContext.Provider>;
}

export function useCollections(): UseCollectionsResult {
  const ctx = useContext(CollectionsContext);
  if (ctx === null) {
    throw new Error('useCollections must be used within CollectionsProvider');
  }
  return ctx;
}
