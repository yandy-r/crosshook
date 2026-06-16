import { TooltipProvider } from '@radix-ui/react-tooltip';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CommandArgumentsPanel } from '@/components/CommandArgumentsPanel';
import SteamLaunchOptionsPanel from '@/components/SteamLaunchOptionsPanel';
import { makeCommandArgumentCatalogPayload } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';
import type { LaunchCommandArguments } from '@/types/launch-command-arguments';
import { DEFAULT_LAUNCH_COMMAND_ARGUMENTS } from '@/types/launch-command-arguments';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

vi.mock('@/hooks/useCapabilityGate', () => ({
  useCapabilityGate: () => ({ state: 'available', rationale: '' }),
}));

const catalog = makeCommandArgumentCatalogPayload();

function renderCommandArgumentsPanel(overrides: Partial<React.ComponentProps<typeof CommandArgumentsPanel>> = {}) {
  const onToggleArgument = vi.fn();
  const onUpdateCustomArgs = vi.fn();

  const view = renderWithMocks(
    <TooltipProvider>
      <CommandArgumentsPanel
        method="proton_run"
        commandArguments={DEFAULT_LAUNCH_COMMAND_ARGUMENTS}
        catalog={catalog}
        onToggleArgument={onToggleArgument}
        onUpdateCustomArgs={onUpdateCustomArgs}
        {...overrides}
      />
    </TooltipProvider>
  );

  return { ...view, onToggleArgument, onUpdateCustomArgs };
}

describe('CommandArgumentsPanel', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;
  let uuidCounter = 0;

  beforeEach(() => {
    uuidCounter = 0;
    vi.stubGlobal('crypto', {
      randomUUID: () => `test-uuid-${++uuidCounter}`,
    });
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('shows loading state when catalog is null', () => {
    renderCommandArgumentsPanel({ catalog: null });
    expect(screen.getByText('Loading command arguments...')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders curated argument toggles for supported methods', () => {
    renderCommandArgumentsPanel();
    expect(screen.getByRole('heading', { name: 'Command Arguments' })).toBeInTheDocument();
    expect(screen.getByLabelText('Force Vulkan renderer')).toBeInTheDocument();
    expect(screen.getByLabelText('Skip in-game launcher')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows method warning for native launch method', () => {
    renderCommandArgumentsPanel({ method: 'native' });
    expect(screen.getByText(/Command arguments are only editable when the profile method is/)).toBeInTheDocument();
    expect(screen.getByLabelText('Force Vulkan renderer')).toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('calls onToggleArgument when a curated argument is toggled', async () => {
    const user = userEvent.setup();
    const { onToggleArgument } = renderCommandArgumentsPanel();

    await user.click(screen.getByLabelText('Force Vulkan renderer'));
    expect(onToggleArgument).toHaveBeenCalledWith('force_vulkan', true);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('adds, edits, and removes custom tokens in order', async () => {
    const user = userEvent.setup();
    const { onUpdateCustomArgs } = renderCommandArgumentsPanel({
      commandArguments: {
        enabled_argument_ids: [],
        custom_args: ['--first'],
      },
    });

    const tokenInput = screen.getByLabelText('Token');
    expect(tokenInput).toHaveValue('--first');

    await user.click(screen.getByRole('button', { name: 'Add token' }));
    expect(onUpdateCustomArgs).toHaveBeenLastCalledWith(['--first', '']);

    await user.clear(tokenInput);
    await user.type(tokenInput, '--updated');
    expect(onUpdateCustomArgs).toHaveBeenLastCalledWith(['--updated', '']);

    await user.click(screen.getByRole('button', { name: 'Remove custom token 1' }));
    expect(onUpdateCustomArgs).toHaveBeenLastCalledWith(['']);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('moves custom tokens up and down', async () => {
    const user = userEvent.setup();
    const { onUpdateCustomArgs } = renderCommandArgumentsPanel({
      commandArguments: {
        enabled_argument_ids: [],
        custom_args: ['--alpha', '--beta'],
      },
    });

    await user.click(screen.getByRole('button', { name: 'Move custom token 2 up' }));
    expect(onUpdateCustomArgs).toHaveBeenLastCalledWith(['--beta', '--alpha']);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('does not autosave blank custom token rows with validation errors', async () => {
    const user = userEvent.setup();
    const { onUpdateCustomArgs } = renderCommandArgumentsPanel({
      commandArguments: {
        enabled_argument_ids: [],
        custom_args: ['valid'],
      },
    });

    await user.click(screen.getByRole('button', { name: 'Add token' }));
    const callsBeforeBlankEdit = onUpdateCustomArgs.mock.calls.length;

    const tokenInputs = screen.getAllByLabelText('Token');
    await user.type(tokenInputs[1], '   ');
    expect(onUpdateCustomArgs.mock.calls.length).toBe(callsBeforeBlankEdit);
    expect(screen.getByRole('alert')).toHaveTextContent(/cannot be empty or whitespace-only/i);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});

describe('SteamLaunchOptionsPanel command arguments', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
    vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function renderSteamPanel(commandArguments: LaunchCommandArguments) {
    return renderWithMocks(<SteamLaunchOptionsPanel enabledOptionIds={[]} commandArguments={commandArguments} />);
  }

  function getSteamPreview(): HTMLElement {
    const preview = document.querySelector('.crosshook-steam-launch-options__preview');
    if (!(preview instanceof HTMLElement)) {
      throw new Error('Steam launch options preview element not found');
    }
    return preview;
  }

  it('appends resolved command arguments after %command% in the preview line', async () => {
    renderSteamPanel({
      enabled_argument_ids: ['force_vulkan'],
      custom_args: ['--custom-flag'],
    });

    await waitFor(() => {
      expect(getSteamPreview()).toHaveTextContent('%command% -force_vulkan --custom-flag');
    });
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows only the prefix when no command arguments are configured', async () => {
    renderSteamPanel(DEFAULT_LAUNCH_COMMAND_ARGUMENTS);

    await waitFor(() => {
      expect(getSteamPreview()).toHaveTextContent('%command%');
    });
    expect(getSteamPreview().textContent?.trim()).toBe('%command%');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('refreshes the preview when command arguments change', async () => {
    const { rerender } = renderWithMocks(
      <SteamLaunchOptionsPanel
        enabledOptionIds={[]}
        commandArguments={{ enabled_argument_ids: ['skip_launcher'], custom_args: [] }}
      />
    );

    await waitFor(() => {
      expect(getSteamPreview()).toHaveTextContent('-skip_launcher');
    });

    rerender(
      <SteamLaunchOptionsPanel
        enabledOptionIds={[]}
        commandArguments={{ enabled_argument_ids: [], custom_args: ['+set', 'gfx'] }}
      />
    );

    await waitFor(() => {
      expect(getSteamPreview()).toHaveTextContent('+set gfx');
    });
    expect(getSteamPreview()).not.toHaveTextContent('-skip_launcher');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
