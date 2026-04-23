import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useCommandPalette } from '@/hooks/useCommandPalette';
import type { CommandPaletteCommand } from '@/lib/commands';
import { renderWithMocks } from '@/test/render';
import { CommandPalette } from '../CommandPalette';

interface PaletteHarnessProps {
  commands: readonly CommandPaletteCommand[];
  onClose?: () => void;
  onExecute?: (command: CommandPaletteCommand) => void | Promise<void>;
}

function PaletteHarness({ commands, onClose, onExecute }: PaletteHarnessProps) {
  const { open, query, filteredCommands, activeId, setQuery, moveActive, closePalette } = useCommandPalette({
    initialOpen: true,
    commands,
    onExecuteCommand: async (command) => {
      await onExecute?.(command);
    },
  });

  const handleClose = () => {
    if (!open) {
      return;
    }
    closePalette();
    onClose?.();
  };

  const handleQuery = (next: string) => {
    setQuery(next);
  };

  const handleExecute = async (command: CommandPaletteCommand) => {
    await onExecute?.(command);
  };

  return (
    <CommandPalette
      open={open}
      query={query}
      commands={filteredCommands}
      activeId={activeId}
      onClose={handleClose}
      onQueryChange={handleQuery}
      onMoveActive={moveActive}
      onExecuteCommand={handleExecute}
    />
  );
}

const COMMANDS: readonly CommandPaletteCommand[] = [
  {
    id: 'route:library',
    action: 'route',
    route: 'library',
    title: 'Go to Library',
    subtitle: 'Browse your library and favorites.',
    icon: 'library',
    keywords: ['library', 'games'],
  },
  {
    id: 'route:profiles',
    action: 'route',
    route: 'profiles',
    title: 'Go to Profiles',
    subtitle: 'Edit existing launch profiles.',
    icon: 'profiles',
    keywords: ['profiles', 'edit'],
  },
  {
    id: 'profile:launch-current:Test Game',
    action: 'launch_profile',
    profileName: 'Test Game',
    title: 'Launch Test Game',
    subtitle: 'Load a profile and switch to launch.',
    icon: 'launch',
  },
  {
    id: 'route:settings',
    action: 'route',
    route: 'settings',
    title: 'Go to Settings',
    subtitle: 'Open app-level settings.',
    icon: 'settings',
    keywords: ['settings', 'setup'],
  },
] as const;

describe('CommandPalette', () => {
  const renderPalette = (
    commands = COMMANDS,
    onExecute?: PaletteHarnessProps['onExecute'],
    onClose?: PaletteHarnessProps['onClose']
  ) => renderWithMocks(<PaletteHarness commands={commands} onExecute={onExecute} onClose={onClose} />);

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('does not render when closed', () => {
    renderWithMocks(
      <CommandPalette
        open={false}
        query=""
        commands={COMMANDS}
        activeId={COMMANDS[0].id}
        onClose={() => {}}
        onQueryChange={() => {}}
        onMoveActive={() => {}}
        onExecuteCommand={() => Promise.resolve()}
      />
    );

    expect(screen.queryByRole('heading', { name: 'Command Palette' })).not.toBeInTheDocument();
  });

  it('filters command results with substring input', async () => {
    const user = userEvent.setup();
    renderPalette();

    const search = screen.getByRole('searchbox', { name: 'Search commands' });
    await user.type(search, 'set');

    expect(screen.getByRole('button', { name: /Go to Settings/i })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Go to Library/i })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Go to Profiles/i })).not.toBeInTheDocument();
  });

  it('shows empty-state messaging when no command matches', async () => {
    const user = userEvent.setup();
    renderPalette();

    const search = screen.getByRole('searchbox', { name: 'Search commands' });
    await user.type(search, 'totally-missing');

    expect(screen.getByRole('status')).toHaveTextContent('No commands found');
  });

  it('supports arrow-key wrap navigation', async () => {
    const user = userEvent.setup();
    renderPalette();

    const search = screen.getByRole('searchbox', { name: 'Search commands' });
    await user.click(search);
    await user.keyboard('{ArrowUp}');
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Go to Settings/i })).toHaveAttribute('aria-current', 'true');
    });

    await user.keyboard('{ArrowDown}');
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /Go to Library/i })).toHaveAttribute('aria-current', 'true');
    });
  });

  it('executes active command on Enter', async () => {
    const onExecute = vi.fn();
    const user = userEvent.setup();
    renderPalette(COMMANDS, onExecute);

    const search = screen.getByRole('searchbox', { name: 'Search commands' });
    await user.click(search);
    await user.keyboard('{Enter}');

    expect(onExecute).toHaveBeenCalledTimes(1);
    expect(onExecute).toHaveBeenCalledWith(COMMANDS[0]);
  });

  it('closes when Escape is pressed', async () => {
    const onClose = vi.fn();
    const user = userEvent.setup();
    renderPalette(COMMANDS, undefined, onClose);

    const search = screen.getByRole('searchbox', { name: 'Search commands' });
    await user.click(search);
    await user.keyboard('{Escape}');

    expect(onClose).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('searchbox', { name: 'Search commands' })).not.toBeInTheDocument();
  });

  it('executes command when clicked', async () => {
    const onExecute = vi.fn();
    const user = userEvent.setup();
    renderPalette(COMMANDS, onExecute);

    await user.click(screen.getByRole('button', { name: /Go to Profiles/i }));
    expect(onExecute).toHaveBeenCalledWith(COMMANDS[1]);
  });
});
