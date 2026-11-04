import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { INSTANCE_UNGUARDED_DISMISS_KEY, InstanceGuardBanner } from '@/components/layout/InstanceGuardBanner';
import { fetchStartupReport } from '@/lib/startupReport';
import { renderWithMocks } from '@/test/render';
import { makeStartupReport } from '@/test/startupReportFixture';

vi.mock('@/lib/ipc', async () => {
  // Load inside the hoisted mock factory so the IPC mock is registered first.
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

function reportWith(instanceUnguarded: boolean) {
  const base = makeStartupReport();
  return makeStartupReport({ degraded: { ...base.degraded, instance_unguarded: instanceUnguarded } });
}

beforeEach(() => {
  sessionStorage.clear();
});

describe('InstanceGuardBanner', () => {
  it('renders nothing when the instance is guarded', async () => {
    const getReport = vi.fn(async () => reportWith(false));
    const { container } = renderWithMocks(<InstanceGuardBanner />, {
      handlerOverrides: { get_startup_report: getReport },
    });
    await waitFor(() => expect(getReport).toHaveBeenCalled());
    await fetchStartupReport();
    expect(screen.queryByRole('status')).toBeNull();
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the warning when the instance is unguarded', async () => {
    renderWithMocks(<InstanceGuardBanner />, {
      handlerOverrides: { get_startup_report: async () => reportWith(true) },
    });
    const banner = await screen.findByRole('status', { name: 'Single-instance warning' });
    expect(banner).toHaveTextContent('CrossHook could not confirm it is the only running copy');
    expect(banner).toHaveTextContent('Avoid opening a second CrossHook window.');
  });

  it('renders nothing when the startup report fails', async () => {
    const getReport = vi.fn(async () => {
      throw new Error('[test-mock] forced failure');
    });
    const { container } = renderWithMocks(<InstanceGuardBanner />, {
      handlerOverrides: { get_startup_report: getReport },
    });
    await waitFor(() => expect(getReport).toHaveBeenCalled());
    await expect(fetchStartupReport()).rejects.toThrow();
    expect(container).toBeEmptyDOMElement();
  });

  it('dismiss hides the banner and persists for the session', async () => {
    const user = userEvent.setup();
    const first = renderWithMocks(<InstanceGuardBanner />, {
      handlerOverrides: { get_startup_report: async () => reportWith(true) },
    });
    const banner = await screen.findByRole('status', { name: 'Single-instance warning' });
    expect(sessionStorage.getItem(INSTANCE_UNGUARDED_DISMISS_KEY)).toBeNull();

    await user.click(screen.getByRole('button', { name: 'Dismiss single-instance warning' }));
    expect(banner).not.toBeInTheDocument();
    expect(sessionStorage.getItem(INSTANCE_UNGUARDED_DISMISS_KEY)).toBe('1');
    first.unmount();

    const getReport = vi.fn(async () => reportWith(true));
    const second = renderWithMocks(<InstanceGuardBanner />, {
      handlerOverrides: { get_startup_report: getReport },
    });
    await waitFor(() => expect(getReport).toHaveBeenCalled());
    await fetchStartupReport();
    expect(screen.queryByRole('status')).toBeNull();
    expect(second.container).toBeEmptyDOMElement();
  });
});
