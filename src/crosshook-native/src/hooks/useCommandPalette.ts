import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  type CommandPaletteCommand,
  type CommandPaletteCommandId,
  filterCommandPaletteCommands,
  isCommandPaletteCommandEnabled,
} from '@/lib/commands';

export interface UseCommandPaletteOptions {
  commands: readonly CommandPaletteCommand[];
  /** Used by {@link UseCommandPaletteReturn.executeActive} only. Omit if execution is handled via {@link CommandPalette} `onExecuteCommand`. */
  onExecuteCommand?: (command: CommandPaletteCommand) => void | Promise<void>;
  initialOpen?: boolean;
  initialQuery?: string;
}

export interface UseCommandPaletteReturn {
  open: boolean;
  query: string;
  filteredCommands: CommandPaletteCommand[];
  activeIndex: number;
  activeId: CommandPaletteCommandId | null;
  openPalette: () => void;
  closePalette: () => void;
  setQuery: (query: string) => void;
  moveActive: (delta: number) => void;
  reset: () => void;
  executeActive: () => Promise<boolean>;
}

function clampWrappedIndex(index: number, length: number): number {
  return ((index % length) + length) % length;
}

export function useCommandPalette({
  commands,
  onExecuteCommand,
  initialOpen = false,
  initialQuery = '',
}: UseCommandPaletteOptions): UseCommandPaletteReturn {
  const [open, setOpen] = useState(initialOpen);
  const [query, setQueryState] = useState(initialQuery);
  const [activeId, setActiveId] = useState<CommandPaletteCommandId | null>(null);

  const filteredCommands = useMemo(() => filterCommandPaletteCommands(commands, query), [commands, query]);
  const enabledCommands = useMemo(() => filteredCommands.filter(isCommandPaletteCommandEnabled), [filteredCommands]);
  const activeIndex = useMemo(() => {
    if (activeId === null) {
      return -1;
    }

    return filteredCommands.findIndex((command) => command.id === activeId);
  }, [activeId, filteredCommands]);

  const reset = useCallback(() => {
    setQueryState('');
    setActiveId(null);
  }, []);

  const openPalette = useCallback(() => {
    setOpen(true);
  }, []);

  const closePalette = useCallback(() => {
    setOpen(false);
    reset();
  }, [reset]);

  const setQuery = useCallback((nextQuery: string) => {
    setQueryState(nextQuery);
  }, []);

  useEffect(() => {
    if (!open) {
      return;
    }

    const nextActive = enabledCommands[0] ?? null;
    if (nextActive === null) {
      if (activeId !== null) {
        setActiveId(null);
      }
      return;
    }

    if (activeId === null) {
      setActiveId(nextActive.id);
      return;
    }

    const activeCommand = enabledCommands.find((command) => command.id === activeId);
    if (!activeCommand) {
      setActiveId(nextActive.id);
    }
  }, [activeId, enabledCommands, open]);

  const moveActive = useCallback(
    (delta: number) => {
      if (!open) {
        return;
      }

      if (enabledCommands.length === 0) {
        setActiveId(null);
        return;
      }

      const currentIndex = activeId === null ? -1 : enabledCommands.findIndex((command) => command.id === activeId);
      const nextIndex =
        currentIndex === -1
          ? delta >= 0
            ? 0
            : enabledCommands.length - 1
          : clampWrappedIndex(currentIndex + delta, enabledCommands.length);

      setActiveId(enabledCommands[nextIndex].id);
    },
    [activeId, enabledCommands, open]
  );

  const executeActive = useCallback(async (): Promise<boolean> => {
    if (onExecuteCommand == null) {
      return false;
    }

    if (activeId === null) {
      return false;
    }

    const activeCommand = enabledCommands.find((command) => command.id === activeId);
    if (!activeCommand) {
      return false;
    }

    await onExecuteCommand(activeCommand);
    closePalette();
    return true;
  }, [activeId, closePalette, enabledCommands, onExecuteCommand]);

  return {
    open,
    query,
    filteredCommands,
    activeIndex,
    activeId,
    openPalette,
    closePalette,
    setQuery,
    moveActive,
    reset,
    executeActive,
  };
}
