import { screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithMocks } from '@/test/render';
import { DEFAULT_APP_SETTINGS } from '@/types/settings';
import { SettingsPanel } from '../SettingsPanel';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

describe('SettingsPanel', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.spyOn(console, 'debug').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function renderPanel() {
    return renderWithMocks(
      <SettingsPanel
        settings={DEFAULT_APP_SETTINGS}
        onPersistSettings={async () => {}}
        recentFiles={{ gamePaths: [], trainerPaths: [], dllPaths: [] }}
        targetHomePath=""
        steamClientInstallPath=""
        onAutoLoadLastProfileChange={() => {}}
      />
    );
  }

  it('renders the App preferences dashboard panel section', () => {
    const { container } = renderPanel();

    expect(screen.getByRole('heading', { level: 2, name: 'App preferences and storage' })).toBeInTheDocument();
    expect(container.querySelector('.crosshook-settings-panel')).toBeInTheDocument();
    expect(container.querySelector('.crosshook-dashboard-panel-section')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders the settings grid with both columns', () => {
    const { container } = renderPanel();

    expect(container.querySelector('.crosshook-settings-grid')).toBeInTheDocument();
    expect(container.querySelector('.crosshook-settings-column')).toBeInTheDocument();
  });

  it('renders the 10 synchronously-visible CollapsibleSection sub-sections', () => {
    renderPanel();

    // ManageLaunchersSection conditionally renders after an async list_launchers
    // IPC call resolves; the remaining 10 sections are synchronously present.
    const syncSectionTitles = [
      'Startup',
      'New profile defaults',
      'Runner',
      'Proton manager defaults',
      'Logging and UI',
      'Prefix Dependencies',
      'Profiles',
      'Prefix Storage Health',
      'Diagnostic Export',
      'SteamGridDB',
    ];

    for (const title of syncSectionTitles) {
      expect(screen.getByText(title)).toBeInTheDocument();
    }

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders ManageLaunchersSection after async list_launchers resolves', async () => {
    renderPanel();

    // ManageLaunchersSection is the 11th section; it fetches on mount and only
    // renders when launchers are present. The mock returns a synthetic launcher.
    expect(await screen.findByText('Manage Launchers')).toBeInTheDocument();
  });

  it('renders the status chips for last profile and recent limit', () => {
    renderPanel();

    expect(screen.getByText('Last profile:')).toBeInTheDocument();
    expect(screen.getByText('none')).toBeInTheDocument();
    expect(screen.getByText('Recent limit:')).toBeInTheDocument();
  });

  it('renders the recent files column', () => {
    renderPanel();

    expect(screen.getByText('Recent Files')).toBeInTheDocument();
  });
});
