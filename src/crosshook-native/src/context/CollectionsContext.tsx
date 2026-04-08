import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from 'react';

import { callCommand } from '@/lib/ipc';
import type { CollectionRow } from '@/types/collections';

export interface UseCollectionsResult {
  collections: CollectionRow[];
  error: string | null;
  isListing: boolean;
  creatingName: string | null;
  deletingId: string | null;
  renamingId: string | null;
  refresh: () => Promise<void>;
  createCollection: (name: string, description?: string | null) => Promise<string | null>;
  deleteCollection: (collectionId: string) => Promise<boolean>;
  renameCollection: (collectionId: string, newName: string) => Promise<boolean>;
  updateDescription: (collectionId: string, description: string | null) => Promise<boolean>;
  addProfile: (collectionId: string, profileName: string) => Promise<boolean>;
  removeProfile: (collectionId: string, profileName: string) => Promise<boolean>;
  listMembers: (collectionId: string) => Promise<string[]>;
  collectionsForProfile: (profileName: string) => Promise<CollectionRow[]>;
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
    async (name: string, description: string | null = null): Promise<string | null> => {
      setCreatingName(name);
      setError(null);
      try {
        const id = await callCommand<string>('collection_create', { name });
        if (id !== null && description !== null && description.trim() !== '') {
          try {
            await callCommand<null>('collection_update_description', {
              collectionId: id,
              description: description.trim(),
            });
          } catch (err) {
            setError(err instanceof Error ? err.message : String(err));
          }
        }
        await refresh();
        return id;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return null;
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
    async (collectionId: string, newName: string): Promise<boolean> => {
      setRenamingId(collectionId);
      setError(null);
      try {
        await callCommand<null>('collection_rename', { collectionId, newName });
        await refresh();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      } finally {
        setRenamingId(null);
      }
    },
    [refresh]
  );

  const updateDescription = useCallback(
    async (collectionId: string, description: string | null): Promise<boolean> => {
      setError(null);
      try {
        await callCommand<null>('collection_update_description', { collectionId, description });
        await refresh();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      }
    },
    [refresh]
  );

  const addProfile = useCallback(
    async (collectionId: string, profileName: string): Promise<boolean> => {
      setError(null);
      try {
        await callCommand<null>('collection_add_profile', { collectionId, profileName });
        await refresh();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
      }
    },
    [refresh]
  );

  const removeProfile = useCallback(
    async (collectionId: string, profileName: string): Promise<boolean> => {
      setError(null);
      try {
        await callCommand<null>('collection_remove_profile', { collectionId, profileName });
        await refresh();
        return true;
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        return false;
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
