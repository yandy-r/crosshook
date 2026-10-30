import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { callCommand } from '@/lib/ipc';
import { PrefixDepsPanel } from '../PrefixDepsPanel';

type EventHandler = (event: { payload: unknown }) => void;

const eventHandlers = new Map<string, EventHandler[]>();

vi.mock('@/lib/ipc', () => ({
  callCommand: vi.fn(),
}));

vi.mock('@/lib/events', () => ({
  subscribeEvent: vi.fn(async (name: string, handler: EventHandler) => {
    const handlers = eventHandlers.get(name) ?? [];
    handlers.push(handler);
    eventHandlers.set(name, handlers);
    return () => {};
  }),
}));

const callCommandMock = vi.mocked(callCommand);

const CLEAN_REPAIR_STATUS = {
  required: false,
  state: null,
  last_error: null,
  in_progress: false,
  last_recovery: null,
};

function mockRepairStatus(status: Record<string, unknown>): void {
  const fallback = callCommandMock.getMockImplementation();
  callCommandMock.mockImplementation(async (name, args) => {
    if (name === 'get_prefix_version_repair_status') return { ...CLEAN_REPAIR_STATUS, ...status };
    return fallback?.(name, args);
  });
}

describe('PrefixDepsPanel', () => {
  let repairRequired: boolean;

  beforeEach(() => {
    eventHandlers.clear();
    repairRequired = true;
    callCommandMock.mockReset();
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'get_dependency_status') {
        return [
          {
            package_name: 'dotnet48',
            state: 'missing',
            checked_at: null,
            installed_at: null,
            last_error: null,
          },
        ];
      }
      if (name === 'get_prefix_version_repair_status') {
        return repairRequired
          ? { ...CLEAN_REPAIR_STATUS, required: true, state: 'failed', last_error: 'restore timed out' }
          : CLEAN_REPAIR_STATUS;
      }
      if (name === 'repair_prefix_windows_version') {
        repairRequired = false;
        return CLEAN_REPAIR_STATUS;
      }
      throw new Error(`unexpected command: ${name}`);
    });
  });

  it('blocks dependency mutations while prefix repair is required', async () => {
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={['dotnet48']} />);

    const alert = await screen.findByRole('alert');
    expect(alert).toHaveTextContent('Prefix repair required');
    expect(alert).toHaveTextContent('restore timed out');
    expect(screen.getByRole('button', { name: 'Repair Prefix' })).toBeEnabled();
    expect(screen.getByRole('button', { name: 'Check Now' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Install' })).toBeDisabled();
  });

  it('repairs the prefix through IPC and refreshes the displayed state', async () => {
    const user = userEvent.setup();
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={['dotnet48']} />);
    await screen.findByText('Prefix repair required');

    await user.click(screen.getByRole('button', { name: 'Repair Prefix' }));

    expect(callCommandMock).toHaveBeenCalledWith('repair_prefix_windows_version', {
      profileName: 'Ghost',
      prefixPath: '/prefix',
    });
    await waitFor(() => expect(screen.queryByText('Prefix repair required')).not.toBeInTheDocument());
  });

  it('reloads repair status after every dependency completion outcome', async () => {
    repairRequired = false;
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={['dotnet48']} />);
    await waitFor(() => {
      expect(callCommandMock.mock.calls.filter(([name]) => name === 'get_prefix_version_repair_status')).toHaveLength(
        1
      );
    });

    await act(async () => {
      for (const handler of eventHandlers.get('prefix-dep-complete') ?? []) {
        handler({
          payload: {
            profile_name: 'Ghost',
            prefix_path: '/prefix',
            succeeded: false,
            exit_code: 0,
            install_succeeded: true,
            install_exit_code: 0,
            restore_state: 'failed',
            restore_error: 'restore timed out',
          },
        });
      }
    });

    await waitFor(() => {
      expect(callCommandMock.mock.calls.filter(([name]) => name === 'get_prefix_version_repair_status')).toHaveLength(
        2
      );
    });
  });

  it('renders a neutral operational state when no dependencies or repair are present', async () => {
    repairRequired = false;
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={[]} />);

    expect(await screen.findByText(/No prefix dependencies are declared/)).toHaveAttribute('role', 'status');
  });

  it('shows an automatic restore as a status note, not an alert', async () => {
    mockRepairStatus({ last_recovery: { outcome: 'restored', message: null } });
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={['dotnet48']} />);

    const note = await screen.findByText(/CrossHook restored this prefix's Windows compatibility version/);
    expect(note).toHaveAttribute('role', 'status');
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('button', { name: 'Install' })).toBeEnabled());
  });

  it('keeps the blocking alert when automatic recovery failed', async () => {
    mockRepairStatus({
      required: true,
      state: 'failed',
      last_error: 'restore timed out',
      last_recovery: { outcome: 'failed', message: 'restore timed out' },
    });
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={['dotnet48']} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('Prefix repair required');
    expect(screen.queryByText(/CrossHook restored this prefix/)).not.toBeInTheDocument();
  });

  it('shows no repair banner while a job holds the prefix', async () => {
    mockRepairStatus({ required: true, state: 'pending', in_progress: true });
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={['dotnet48']} />);

    // The dependency badge and repair status are applied in the same update.
    expect(await screen.findByText('dotnet48: Missing')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Install' })).toBeDisabled();
    expect(screen.queryByText('Prefix repair required')).not.toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    expect(screen.getByText(/Another operation is using this prefix/)).toHaveAttribute('role', 'status');
  });

  it('shows no busy note while a job holds the prefix without a repair row', async () => {
    mockRepairStatus({ in_progress: true });
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={['dotnet48']} />);

    expect(await screen.findByText('dotnet48: Missing')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('button', { name: 'Install' })).toBeEnabled());
    expect(screen.queryByText(/Another operation is using this prefix/)).not.toBeInTheDocument();
  });

  it('disables installation but allows status retry when repair safety is unknown', async () => {
    callCommandMock.mockImplementation(async (name) => {
      if (name === 'get_dependency_status') {
        return [
          {
            package_name: 'dotnet48',
            state: 'missing',
            checked_at: null,
            installed_at: null,
            last_error: null,
          },
        ];
      }
      if (name === 'get_prefix_version_repair_status') throw new Error('repair journal unavailable');
      throw new Error(`unexpected command: ${name}`);
    });
    render(<PrefixDepsPanel profileName="Ghost" prefixPath="/prefix" requiredPackages={['dotnet48']} />);

    expect(await screen.findByRole('alert')).toHaveTextContent('repair journal unavailable');
    expect(screen.getByRole('button', { name: 'Install' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Check Now' })).toBeEnabled();
  });
});
