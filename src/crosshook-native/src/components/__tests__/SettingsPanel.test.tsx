import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { renderWithMocks } from '@/test/render';
import type { AppSettingsData } from '@/types';
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

  function renderPanel({
    onPersistSettings = async () => {},
    handlerOverrides = {},
  }: {
    onPersistSettings?: (patch: Partial<AppSettingsData>) => Promise<void>;
    handlerOverrides?: Parameters<typeof renderWithMocks>[1]['handlerOverrides'];
  } = {}) {
    return renderWithMocks(
      <SettingsPanel
        settings={DEFAULT_APP_SETTINGS}
        onPersistSettings={onPersistSettings}
        recentFiles={{ gamePaths: [], trainerPaths: [], dllPaths: [] }}
        targetHomePath=""
        steamClientInstallPath=""
        onAutoLoadLastProfileChange={() => {}}
      />,
      { handlerOverrides }
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

  it('renders the 11 synchronously-visible CollapsibleSection sub-sections', () => {
    renderPanel();

    // ManageLaunchersSection conditionally renders after an async list_launchers
    // IPC call resolves; the remaining 11 sections are synchronously present.
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
      'Advanced',
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

  it('persists runner preference changes from the Runner section', async () => {
    const user = userEvent.setup();
    const onPersistSettings = vi.fn().mockResolvedValue(undefined);
    renderPanel({ onPersistSettings });

    await user.click(screen.getByText('Runner'));
    await user.click(screen.getByLabelText('umu GAMEID lookup'));
    await user.click(await screen.findByRole('option', { name: 'Enabled' }));

    expect(onPersistSettings).toHaveBeenCalledWith({ umu_database_lookup: 'enabled' });
  });

  it('refreshes the umu database with live status feedback', async () => {
    const user = userEvent.setup();
    const refreshUmuDatabase = vi.fn().mockResolvedValue({
      refreshed: true,
      cached_at: '2026-06-07T12:00:00.000Z',
      source_url: 'https://example.test/umu-database.csv',
      reason: 'test refresh',
    });
    renderPanel({ handlerOverrides: { refresh_umu_database: refreshUmuDatabase } });

    await user.click(screen.getByText('Runner'));
    const refreshButton = screen.getByRole('button', { name: 'Refresh umu protonfix database' });
    expect(refreshButton).toHaveAccessibleDescription('Not refreshed this session');
    await user.click(refreshButton);

    await waitFor(() => expect(refreshUmuDatabase).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(/Last refreshed:/)).toBeInTheDocument();
  });

  it('clears the umu GAMEID lookup cache with live status feedback', async () => {
    const user = userEvent.setup();
    const clearLookupCache = vi.fn().mockResolvedValue(7);
    renderPanel({ handlerOverrides: { clear_umu_gameid_lookup_cache: clearLookupCache } });

    await user.click(screen.getByText('Advanced'));
    const clearButton = screen.getByRole('button', { name: 'Clear lookup cache' });
    expect(clearButton).toHaveAccessibleDescription('Lookup cache not cleared this session');
    await user.click(clearButton);

    await waitFor(() => expect(clearLookupCache).toHaveBeenCalledTimes(1));
    expect(screen.getByText('Cleared 7 cached rows.')).toBeInTheDocument();
  });
});
