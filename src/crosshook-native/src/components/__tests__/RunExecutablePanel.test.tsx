import { TooltipProvider } from '@radix-ui/react-tooltip';
import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { emitMockEvent } from '@/lib/events';
import { renderWithMocks } from '@/test/render';
import { RunExecutablePanel } from '../RunExecutablePanel';

// vi.mock factories are hoisted above static imports, so the mock registry is imported lazily.
vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

function renderPanel() {
  return renderWithMocks(
    <TooltipProvider>
      <PreferencesProvider>
        <RunExecutablePanel protonInstalls={[]} protonInstallsError={null} />
      </PreferencesProvider>
    </TooltipProvider>
  );
}

describe('RunExecutablePanel', () => {
  it('streams run output as text into a log region', async () => {
    const user = userEvent.setup();
    renderPanel();

    await user.type(
      await screen.findByPlaceholderText('/mnt/media/setup.exe or /mnt/media/installer.msi'),
      '/mock/games/synthetic-quest/setup.exe'
    );
    await user.type(
      screen.getByPlaceholderText(/steamapps\/common\/Proton - Experimental\/proton/),
      '/mock/proton/GE-Proton/proton'
    );

    await user.click(screen.getByRole('button', { name: 'Run' }));
    const dialog = await screen.findByRole('dialog');
    expect(dialog).toHaveTextContent('stream its output below');
    await user.click(within(dialog).getByRole('button', { name: 'Run' }));

    const log = await screen.findByRole('log', { name: 'Run output' });
    await waitFor(() => expect(screen.getByText('Running')).toBeInTheDocument());

    act(() => {
      emitMockEvent('run-executable-log', { run_id: 'mock-run-1', line: '<b>wine</b> ready' });
    });

    expect(log).toHaveTextContent('<b>wine</b> ready');
    expect(log.querySelector('b')).toBeNull();
  });
});
