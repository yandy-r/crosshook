import {
  createContext,
  type Dispatch,
  type ReactNode,
  type SetStateAction,
  useContext,
  useMemo,
  useState,
} from 'react';

import type { InspectorBodyProps, SelectedGame } from '@/components/layout/routeMetadata';

export type LibraryInspectorHandlers = Pick<InspectorBodyProps, 'onLaunch' | 'onEditProfile' | 'onToggleFavorite'>;

export type InspectorSelectionContextValue = {
  inspectorSelection: SelectedGame | undefined;
  setInspectorSelection: Dispatch<SetStateAction<SelectedGame | undefined>>;
  libraryInspectorHandlers: LibraryInspectorHandlers | undefined;
  setLibraryInspectorHandlers: (handlers: LibraryInspectorHandlers | undefined) => void;
};

const InspectorSelectionContext = createContext<InspectorSelectionContextValue | null>(null);

export function InspectorSelectionProvider({ children }: { children: ReactNode }) {
  const [inspectorSelection, setInspectorSelection] = useState<SelectedGame | undefined>();
  const [libraryInspectorHandlers, setLibraryInspectorHandlers] = useState<LibraryInspectorHandlers | undefined>();

  const value = useMemo<InspectorSelectionContextValue>(
    () => ({
      inspectorSelection,
      setInspectorSelection,
      libraryInspectorHandlers,
      setLibraryInspectorHandlers,
    }),
    [inspectorSelection, libraryInspectorHandlers]
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
