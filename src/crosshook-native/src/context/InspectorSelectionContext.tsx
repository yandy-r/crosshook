import {
  createContext,
  type Dispatch,
  type ReactNode,
  type SetStateAction,
  useCallback,
  useContext,
  useMemo,
  useState,
} from 'react';

import type { InspectorBodyProps, SelectedGame } from '@/components/layout/routeMetadata';

export type LibraryInspectorHandlers = Pick<InspectorBodyProps, 'onLaunch' | 'onEditProfile' | 'onToggleFavorite'>;

export type LibraryShellMode = 'library' | 'detail';

export type InspectorSelectionContextValue = {
  inspectorSelection: SelectedGame | undefined;
  setInspectorSelection: Dispatch<SetStateAction<SelectedGame | undefined>>;
  libraryInspectorHandlers: LibraryInspectorHandlers | undefined;
  setLibraryInspectorHandlers: (handlers: LibraryInspectorHandlers | undefined) => void;
  libraryShellMode: LibraryShellMode;
  setLibraryShellMode: Dispatch<SetStateAction<LibraryShellMode>>;
};

const InspectorSelectionContext = createContext<InspectorSelectionContextValue | null>(null);

export function InspectorSelectionProvider({ children }: { children: ReactNode }) {
  const [inspectorSelection, setInspectorSelection] = useState<SelectedGame | undefined>();
  const [libraryInspectorHandlers, setLibraryInspectorHandlersState] = useState<LibraryInspectorHandlers | undefined>();
  const [libraryShellMode, setLibraryShellModeState] = useState<LibraryShellMode>('library');

  const setLibraryInspectorHandlers = useCallback((handlers: LibraryInspectorHandlers | undefined) => {
    setLibraryInspectorHandlersState((prev) => {
      if (
        prev?.onLaunch === handlers?.onLaunch &&
        prev?.onEditProfile === handlers?.onEditProfile &&
        prev?.onToggleFavorite === handlers?.onToggleFavorite
      ) {
        return prev;
      }
      return handlers;
    });
  }, []);

  const setLibraryShellMode = useCallback<Dispatch<SetStateAction<LibraryShellMode>>>((update) => {
    setLibraryShellModeState((prev) => {
      const next = typeof update === 'function' ? update(prev) : update;
      return prev === next ? prev : next;
    });
  }, []);

  const value = useMemo<InspectorSelectionContextValue>(
    () => ({
      inspectorSelection,
      setInspectorSelection,
      libraryInspectorHandlers,
      setLibraryInspectorHandlers,
      libraryShellMode,
      setLibraryShellMode,
    }),
    [inspectorSelection, libraryInspectorHandlers, libraryShellMode, setLibraryInspectorHandlers, setLibraryShellMode]
  );

  return <InspectorSelectionContext.Provider value={value}>{children}</InspectorSelectionContext.Provider>;
}

export function useInspectorSelection(): InspectorSelectionContextValue {
  const ctx = useContext(InspectorSelectionContext);
  if (ctx === null) {
    throw new Error('useInspectorSelection must be used within an InspectorSelectionProvider.');
  }
  return ctx;
}
