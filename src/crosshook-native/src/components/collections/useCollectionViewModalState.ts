import { useCallback, useState } from 'react';

export interface UseCollectionViewModalStateResult {
  open: boolean;
  collectionId: string | null;
  openForCollection: (id: string) => void;
  close: () => void;
}

export function useCollectionViewModalState(): UseCollectionViewModalStateResult {
  const [open, setOpen] = useState(false);
  const [collectionId, setCollectionId] = useState<string | null>(null);

  const close = useCallback(() => {
    setOpen(false);
    setCollectionId(null);
  }, []);

  const openForCollection = useCallback((id: string) => {
    setCollectionId(id);
    setOpen(true);
  }, []);

  return { open, collectionId, openForCollection, close };
}
