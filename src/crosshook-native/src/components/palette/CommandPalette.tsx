import { type KeyboardEvent, type MouseEvent, useCallback, useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

import {
  BrowseIcon,
  CompatibilityIcon,
  DiscoverIcon,
  HealthIcon,
  HostToolsIcon,
  InfoCircleIcon,
  InstallIcon,
  LaunchIcon,
  LibraryIcon,
  ProfilesIcon,
  ProtonManagerIcon,
  SettingsIcon,
} from '@/components/icons/SidebarIcons';
import { useFocusTrap } from '@/hooks/useFocusTrap';
import {
  type CommandPaletteCommand,
  type CommandPaletteCommandId,
  type CommandPaletteIconId,
  isCommandPaletteCommandEnabled,
} from '@/lib/commands';

export type CommandPaletteItem = CommandPaletteCommand & {
  hint?: string;
  icon?: CommandPaletteIconId;
};

export interface CommandPaletteProps {
  open: boolean;
  query: string;
  commands: readonly CommandPaletteItem[];
  activeId: CommandPaletteCommandId | null;
  onClose: () => void;
  onQueryChange: (query: string) => void;
  onMoveActive: (delta: number) => void;
  onExecuteCommand: (command: CommandPaletteItem) => void | Promise<void>;
  onActiveCommandChange?: (commandId: CommandPaletteCommandId) => void;
}

function resolveCommandIconId(command: CommandPaletteItem): CommandPaletteIconId {
  if (command.icon) {
    return command.icon;
  }

  const haystack = [command.id, command.title, command.subtitle, ...(command.keywords ?? [])].join(' ').toLowerCase();

  if (haystack.includes('setting')) {
    return 'settings';
  }
  if (haystack.includes('proton')) {
    return 'proton_manager';
  }
  if (haystack.includes('host')) {
    return 'host_tools';
  }
  if (haystack.includes('compat')) {
    return 'compatibility';
  }
  if (haystack.includes('health')) {
    return 'health';
  }
  if (haystack.includes('discover') || haystack.includes('community')) {
    return 'discover';
  }
  if (haystack.includes('browse')) {
    return 'browse';
  }
  if (haystack.includes('install')) {
    return 'install';
  }
  if (haystack.includes('launch') || haystack.includes('run')) {
    return 'launch';
  }
  if (haystack.includes('profile')) {
    return 'profiles';
  }

  return 'library';
}

function renderCommandIcon(iconId: CommandPaletteIconId) {
  switch (iconId) {
    case 'browse':
      return <BrowseIcon className="crosshook-palette__row-icon" />;
    case 'compatibility':
      return <CompatibilityIcon className="crosshook-palette__row-icon" />;
    case 'discover':
      return <DiscoverIcon className="crosshook-palette__row-icon" />;
    case 'health':
      return <HealthIcon className="crosshook-palette__row-icon" />;
    case 'host_tools':
      return <HostToolsIcon className="crosshook-palette__row-icon" />;
    case 'info':
      return <InfoCircleIcon className="crosshook-palette__row-icon" />;
    case 'install':
      return <InstallIcon className="crosshook-palette__row-icon" />;
    case 'launch':
      return <LaunchIcon className="crosshook-palette__row-icon" />;
    case 'profiles':
      return <ProfilesIcon className="crosshook-palette__row-icon" />;
    case 'proton_manager':
      return <ProtonManagerIcon className="crosshook-palette__row-icon" />;
    case 'settings':
      return <SettingsIcon className="crosshook-palette__row-icon" />;
    case 'library':
      return <LibraryIcon className="crosshook-palette__row-icon" />;
    default:
      return <InfoCircleIcon className="crosshook-palette__row-icon" />;
  }
}

export function CommandPalette({
  open,
  query,
  commands,
  activeId,
  onClose,
  onQueryChange,
  onMoveActive,
  onExecuteCommand,
  onActiveCommandChange,
}: CommandPaletteProps) {
  const titleId = useId();
  const descriptionId = useId();
  const listId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const portalHostRef = useRef<HTMLElement | null>(null);
  const rowRefs = useRef(new Map<CommandPaletteCommandId, HTMLButtonElement>());
  const [isMounted, setIsMounted] = useState(false);

  const activeCommand = useMemo(
    () => (activeId === null ? null : (commands.find((command) => command.id === activeId) ?? null)),
    [activeId, commands]
  );
  const firstEnabledCommand = useMemo(() => {
    return commands.find(isCommandPaletteCommandEnabled) ?? null;
  }, [commands]);

  useEffect(() => {
    if (typeof document === 'undefined') {
      return;
    }

    const host = document.createElement('div');
    host.className = 'crosshook-modal-portal';
    portalHostRef.current = host;
    document.body.appendChild(host);
    setIsMounted(true);

    return () => {
      host.remove();
      portalHostRef.current = null;
      rowRefs.current.clear();
      setIsMounted(false);
    };
  }, []);

  useEffect(() => {
    if (!open || activeId === null) {
      return;
    }

    rowRefs.current.get(activeId)?.scrollIntoView({ block: 'nearest' });
  }, [activeId, open]);

  const handleClose = useCallback(() => {
    onClose();
  }, [onClose]);

  const { handleKeyDown: handleFocusTrapKeyDown } = useFocusTrap({
    open,
    panelRef,
    onClose: handleClose,
    initialFocusRef: searchRef,
  });

  const handleExecute = useCallback(
    (command: CommandPaletteItem) => {
      if (!isCommandPaletteCommandEnabled(command)) {
        return;
      }

      void onExecuteCommand(command);
    },
    [onExecuteCommand]
  );

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLElement>) => {
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        onMoveActive(event.key === 'ArrowDown' ? 1 : -1);
        return;
      }

      if (event.key === 'Enter') {
        const target = event.target;
        if (target instanceof HTMLElement && target.closest('[data-crosshook-modal-close]')) {
          handleFocusTrapKeyDown(event);
          return;
        }

        const candidate = activeCommand ?? firstEnabledCommand;
        if (candidate && isCommandPaletteCommandEnabled(candidate)) {
          event.preventDefault();
          handleExecute(candidate);
          return;
        }
      }

      handleFocusTrapKeyDown(event);
    },
    [activeCommand, firstEnabledCommand, handleExecute, handleFocusTrapKeyDown, onMoveActive]
  );

  if (!open || !isMounted || !portalHostRef.current) {
    return null;
  }

  const node = (
    <div className="crosshook-modal crosshook-palette" role="presentation">
      <div
        className="crosshook-modal__backdrop"
        aria-hidden="true"
        onMouseDown={(event: MouseEvent<HTMLDivElement>) => {
          if (event.target === event.currentTarget) {
            handleClose();
          }
        }}
      />
      <div
        ref={panelRef}
        className="crosshook-modal__surface crosshook-panel crosshook-focus-scope crosshook-palette__surface"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        data-crosshook-focus-root="modal"
        onKeyDown={handleKeyDown}
      >
        <header className="crosshook-palette__header">
          <div className="crosshook-modal__header">
            <div className="crosshook-palette__header-copy">
              <h2 id={titleId} className="crosshook-palette__title">
                Command Palette
              </h2>
              <p id={descriptionId} className="crosshook-palette__description">
                Jump to routes and actions without leaving the keyboard.
              </p>
            </div>
            <div className="crosshook-modal__header-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--ghost crosshook-modal__close"
                aria-label="Close command palette"
                data-crosshook-modal-close
                onClick={handleClose}
              >
                Close
              </button>
            </div>
          </div>
          <input
            ref={searchRef}
            type="search"
            className="crosshook-palette__search"
            placeholder="Search commands..."
            aria-label="Search commands"
            aria-controls={listId}
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
          />
        </header>
        <div className="crosshook-modal__body crosshook-palette__body">
          {commands.length === 0 ? (
            <div className="crosshook-palette__empty" role="status" aria-live="polite">
              <strong>No commands found</strong>
              <span>{query.trim() ? `No commands match "${query.trim()}".` : 'Try a different search.'}</span>
            </div>
          ) : (
            <ul id={listId} className="crosshook-palette__list" aria-label="Matching commands">
              {commands.map((command) => {
                const enabled = isCommandPaletteCommandEnabled(command);
                const active = command.id === activeId;

                return (
                  <li key={command.id}>
                    <button
                      ref={(element) => {
                        if (element) {
                          rowRefs.current.set(command.id, element);
                          return;
                        }
                        rowRefs.current.delete(command.id);
                      }}
                      type="button"
                      aria-label={command.title}
                      className={`crosshook-palette__row${active ? ' crosshook-palette__row--active' : ''}`}
                      data-crosshook-command-id={command.id}
                      data-state={active ? 'active' : undefined}
                      aria-current={active ? 'true' : undefined}
                      disabled={!enabled}
                      tabIndex={-1}
                      onClick={() => handleExecute(command)}
                      onMouseEnter={() => onActiveCommandChange?.(command.id)}
                    >
                      {renderCommandIcon(resolveCommandIconId(command))}
                      <span className="crosshook-palette__row-copy">
                        <span className="crosshook-palette__row-label">{command.title}</span>
                        {command.subtitle ? (
                          <span className="crosshook-palette__row-description">{command.subtitle}</span>
                        ) : null}
                      </span>
                      {command.hint ? <span className="crosshook-palette__hint-chip">{command.hint}</span> : null}
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>
    </div>
  );

  return createPortal(node, portalHostRef.current);
}

export default CommandPalette;
