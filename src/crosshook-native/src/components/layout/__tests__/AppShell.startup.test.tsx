import { act, screen, waitFor } from '@testing-library/react';
import * as React from 'react';
import { describe, expect, it, vi } from 'vitest';
import { AppShell } from '@/components/layout/AppShell';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider } from '@/context/InspectorSelectionContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { fetchStartupReport } from '@/lib/startupReport';
import { makeProfileDraft } from '@/test/fixtures';
import { renderWithMocks } from '@/test/render';
import { makeStartupReport } from '@/test/startupReportFixture';

vi.mock('@/lib/ipc', async () => {
  // Load inside the hoisted mock factory so the IPC mock is registered first.
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

function AppShellInAppProviders() {
  return (
    <ProfileProvider>
      <ProfileHealthProvider>
        <HostReadinessProvider>
          <CollectionsProvider>
            <InspectorSelectionProvider>
              <AppShell controllerMode={false} />
            </InspectorSelectionProvider>
          </CollectionsProvider>
        </HostReadinessProvider>
      </ProfileHealthProvider>
    </ProfileProvider>
  );
}

describe('AppShell startup', () => {
  it('opens onboarding once when the startup report shows it and there are no profiles', async () => {
    renderWithMocks(<AppShellInAppProviders />, {
      handlerOverrides: {
        get_startup_report: async () => makeStartupReport({ onboarding: { show: true, has_profiles: false } }),
      },
    });

    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Identity & Game' })).toBeInTheDocument();
    expect(screen.getAllByRole('dialog')).toHaveLength(1);
  });

  it('opens exactly one onboarding dialog and auto-loads once under StrictMode double mount', async () => {
    const profileLoad = vi.fn(async () =>
      makeProfileDraft({ game: { name: 'Test Game Alpha', executable_path: '/mock/game.exe' } })
    );
    renderWithMocks(
      <React.StrictMode>
        <AppShellInAppProviders />
      </React.StrictMode>,
      {
        handlerOverrides: {
          get_startup_report: async () =>
            makeStartupReport({
              auto_load_profile: 'Test Game Alpha',
              onboarding: { show: true, has_profiles: false },
            }),
          profile_load: profileLoad,
        },
      }
    );

    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    await waitFor(() => expect(profileLoad).toHaveBeenCalledTimes(1));
    await act(async () => {
      await fetchStartupReport();
    });
    expect(screen.getAllByRole('dialog')).toHaveLength(1);
    expect(profileLoad).toHaveBeenCalledTimes(1);
  });

  it('does not open onboarding when the report has profiles', async () => {
    renderWithMocks(<AppShellInAppProviders />, {
      handlerOverrides: {
        get_startup_report: async () => makeStartupReport({ onboarding: { show: true, has_profiles: true } }),
      },
    });

    await act(async () => {
      await fetchStartupReport();
    });
    expect(screen.getByLabelText('CrossHook navigation')).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
