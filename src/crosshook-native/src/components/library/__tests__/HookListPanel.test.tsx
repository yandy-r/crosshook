import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { HookStage, LaunchHook } from '@/types/profile';
import { HookListPanel } from '../HookListPanel';

function makeHook(overrides: Partial<LaunchHook> = {}): LaunchHook {
  return {
    id: 'hook-1',
    name: 'Backup saves',
    path: '/home/dev/scripts/backup-saves.sh',
    stage: 'pre-launch',
    enabled: true,
    ...overrides,
  };
}

function renderPanel(initialHooks: LaunchHook[] = [], stage: HookStage = 'pre-launch') {
  let hooks = initialHooks;
  const onUpdate = vi.fn((nextHooks: LaunchHook[]) => {
    hooks = nextHooks;
    view.rerender(<HookListPanel hooks={hooks} stage={stage} onUpdate={onUpdate} />);
  });
  const view = render(<HookListPanel hooks={hooks} stage={stage} onUpdate={onUpdate} />);
  return {
    ...view,
    onUpdate,
    get hooks() {
      return hooks;
    },
  };
}

describe('HookListPanel', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  it('adds a stage-aligned hook with a client id', async () => {
    const user = userEvent.setup();
    const { onUpdate } = renderPanel([], 'pre-launch');

    await user.click(screen.getByRole('button', { name: '+ Attach script or DLL' }));

    expect(onUpdate).toHaveBeenCalledWith([
      expect.objectContaining({
        id: expect.any(String),
        name: 'Pre-launch hook',
        path: '',
        stage: 'pre-launch',
        enabled: true,
      }),
    ]);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('toggles, edits, and removes an existing hook', async () => {
    const user = userEvent.setup();
    const panel = renderPanel([makeHook()], 'pre-launch');

    await user.click(screen.getByRole('checkbox', { name: 'Enabled' }));
    expect(panel.hooks[0]).toEqual(expect.objectContaining({ enabled: false, stage: 'pre-launch' }));

    await user.click(screen.getByRole('button', { name: 'Edit Backup saves' }));
    let popover = screen.getByText('Path').closest('.crosshook-hero-detail__hook-popover');
    expect(popover).not.toBeNull();

    await user.clear(within(popover as HTMLElement).getByLabelText('Name'));
    popover = screen.getByText('Path').closest('.crosshook-hero-detail__hook-popover');
    await user.type(within(popover as HTMLElement).getByLabelText('Name'), 'Prepare overlay');
    popover = screen.getByText('Path').closest('.crosshook-hero-detail__hook-popover');
    await user.clear(within(popover as HTMLElement).getByLabelText('Path'));
    popover = screen.getByText('Path').closest('.crosshook-hero-detail__hook-popover');
    await user.type(within(popover as HTMLElement).getByLabelText('Path'), '/home/dev/hooks/prepare-overlay.dll');

    expect(panel.hooks[0]).toEqual(
      expect.objectContaining({
        name: 'Prepare overlay',
        path: '/home/dev/hooks/prepare-overlay.dll',
        stage: 'pre-launch',
      })
    );

    popover = screen.getByText('Path').closest('.crosshook-hero-detail__hook-popover');
    await user.click(within(popover as HTMLElement).getByRole('button', { name: 'Remove' }));
    expect(panel.hooks).toEqual([]);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders invalid hook rows as removable instead of throwing', async () => {
    const user = userEvent.setup();
    const panel = renderPanel([makeHook({ id: '', name: '' })], 'post-exit');

    expect(screen.getByText('Invalid hook')).toBeInTheDocument();
    expect(screen.getByText('Remove this row and attach the hook again.')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Remove' }));

    expect(panel.hooks).toEqual([]);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
