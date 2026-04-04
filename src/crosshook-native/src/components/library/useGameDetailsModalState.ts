import { useCallback, useState } from 'react';

import type { LibraryCardData } from '../../types/library';

export interface UseGameDetailsModalStateResult {
  open: boolean;
  summary: LibraryCardData | null;
  openForCard: (card: LibraryCardData) => void;
  close: () => void;
}

export function useGameDetailsModalState(): UseGameDetailsModalStateResult {
  const [open, setOpen] = useState(false);
  const [summary, setSummary] = useState<LibraryCardData | null>(null);

  const close = useCallback(() => {
    setOpen(false);
    setSummary(null);
  }, []);

  const openForCard = useCallback((card: LibraryCardData) => {
    setSummary(card);
    setOpen(true);
  }, []);

  return { open, summary, openForCard, close };
}
