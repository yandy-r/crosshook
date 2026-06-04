import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLibraryCardData, makeProfileDraft } from '@/test/fixtures';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import type { GameProfile } from '@/types/profile';
import { HeroDetailLaunchTab } from '../HeroDetailLaunchTab';

const profileContextMock = vi.fn();
const preferencesContextMock = vi.fn();
const copyToClipboardMock = vi.fn();
const useLauncherExportMock = vi.fn();
const updateProfileSpy = vi.fn();
const persistProfileDraftSpy = vi.fn();

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

vi.mock('@/context/PreferencesContext', () => ({
  usePreferencesContext: () => preferencesContextMock(),
}));

vi.mock('@/utils/clipboard', () => ({
  copyToClipboard: (text: string) => copyToClipboardMock(text),
}));

vi.mock('@/hooks/useLauncherExport', () => ({
  useLauncherExport: (options: unknown) => useLauncherExportMock(options),
}));

function makeLaunchRequest(): LaunchRequest {
  return {
    method: 'proton_run',
    game_path: '/games/synthetic-quest/game.exe',
    trainer_path: '/trainers/synthetic-quest/trainer.exe',
    trainer_host_path: '/trainers/synthetic-quest/trainer.exe',
    trainer_loading_mode: 'source_directory',
    steam: {
      app_id: '9999001',
      compatdata_path: '/steam/compatdata/9999001',
      proton_path: '/compatibilitytools/proton-ge/proton',
      steam_client_install_path: '/steam/root',
    },
    runtime: {
      prefix_path: '/prefixes/synthetic-quest',
      proton_path: '/compatibilitytools/proton-ge/proton',
      working_directory: '/games/synthetic-quest',
      steam_app_id: '9999001',
    },
    optimizations: { enabled_option_ids: [] },
    launch_trainer_only: false,
    launch_game_only: false,
    profile_name: 'Synthetic Quest',
    custom_env_vars: { DXVK_HUD: 'fps' },
    network_isolation: false,
    gamescope: {
      enabled: false,
      fullscreen: false,
      borderless: false,
      grab_cursor: false,
      force_grab_cursor: false,
      hdr_enabled: false,
      allow_nested: false,
      extra_args: [],
    },
    trainer_gamescope: {
      enabled: false,
      fullscreen: false,
      borderless: false,
      grab_cursor: false,
      force_grab_cursor: false,
      hdr_enabled: false,
      allow_nested: false,
      extra_args: [],
    },
    mangohud: {
      enabled: false,
      gpu_stats: false,
      cpu_stats: false,
      ram: false,
      frametime: false,
      battery: false,
      watt: false,
    },
  };
}

function makePreview(overrides: Partial<LaunchPreview> = {}): LaunchPreview {
  return {
    resolved_method: 'proton_run',
    validation: { issues: [] },
    environment: [{ key: 'DXVK_HUD', value: 'fps', source: 'profile_custom' }],
    cleared_variables: [],
    wrappers: ['gamescope'],
    effective_command: 'gamescope -- /compat/proton run /games/synthetic-quest/game.exe',
    directives_error: null,
    steam_launch_options: null,
    proton_setup: {
      wine_prefix_path: '/prefixes/synthetic-quest',
      compat_data_path: '/steam/compatdata/9999001',
      steam_client_install_path: '/steam/root',
      proton_executable: '/compat/proton',
      umu_run_path: null,
    },
    working_directory: '/games/synthetic-quest',
    game_executable: '/games/synthetic-quest/game.exe',
    game_executable_name: 'game.exe',
    trainer: null,
    generated_at: '2026-04-23T12:00:00.000Z',
    display_text: '',
    umu_decision: null,
    ...overrides,
  };
}

function makeProfile(overrides: Partial<GameProfile> = {}): GameProfile {
  return makeProfileDraft({
    game: { name: 'Synthetic Quest', executable_path: '/games/synthetic-quest/game.exe' },
    trainer: { path: '/trainers/synthetic-quest/trainer.exe', type: '', loading_mode: 'source_directory' },
    runtime: { prefix_path: '/prefixes/synthetic-quest', proton_path: '/compat/proton', working_directory: '' },
    launch: {
      method: 'proton_run',
      optimizations: { enabled_option_ids: [] },
      custom_env_vars: { DXVK_HUD: 'fps' },
    },
    ...overrides,
  });
}

function renderLaunchTab(
  props: Partial<React.ComponentProps<typeof HeroDetailLaunchTab>> = {},
  profileOverrides: Partial<GameProfile> = {}
) {
  const profile = makeProfile(profileOverrides);
  profileContextMock.mockReturnValue({
    profile,
    profileName: 'Synthetic Quest',
    selectedProfile: 'Synthetic Quest',
    profiles: ['Synthetic Quest'],
    updateProfile: updateProfileSpy,
    persistProfileDraft: persistProfileDraftSpy,
    steamClientInstallPath: '/steam/root',
    targetHomePath: '/home/devuser',
  });
  preferencesContextMock.mockReturnValue({
    settings: { umu_preference: 'auto' },
  });

  return render(
    <HeroDetailLaunchTab
      summary={makeLibraryCardData()}
      launchRequest={makeLaunchRequest()}
      previewLoading={false}
      preview={makePreview()}
      previewError={null}
      displayProfileName="Synthetic Quest"
      {...props}
    />
  );
}

describe('HeroDetailLaunchTab', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    copyToClipboardMock.mockResolvedValue(undefined);
    persistProfileDraftSpy.mockResolvedValue({ ok: true });
    useLauncherExportMock.mockReturnValue({
      errorMessage: null,
      statusMessage: null,
      result: null,
      isExporting: false,
      exportLauncher: vi.fn(),
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders the launch, environment, and hooks sections in order', () => {
    renderLaunchTab();

    const headings = screen.getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent);
    expect(headings).toEqual(['Launch command', 'Environment', 'Pre/post hooks']);
    expect(screen.getByText('1 ON')).toBeInTheDocument();
    expect(screen.getByText('No pre/post hooks configured yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add hook (not yet available)' })).toBeDisabled();
  });

  it('hides the environment ON pill when no custom env vars exist', () => {
    const baseProfile = makeProfile();
    renderLaunchTab(
      {},
      {
        launch: {
          ...baseProfile.launch,
          custom_env_vars: {},
        },
      }
    );

    expect(screen.queryByText(/\d+ ON/)).not.toBeInTheDocument();
  });

  it('disables actions when launch request or preview data is unavailable', () => {
    renderLaunchTab({ launchRequest: null, preview: null });

    expect(screen.getByRole('button', { name: 'Dry-run' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Copy' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Launch' })).toBeDisabled();
    expect(
      screen.getByText('Launch preview is unavailable until the game executable is set on this profile.')
    ).toBeInTheDocument();
  });

  it('runs dry-run, copy, export, and launch through existing boundaries', async () => {
    const user = userEvent.setup();
    const onPreviewLaunch = vi.fn();
    const onLaunch = vi.fn();
    const exportLauncher = vi.fn();
    useLauncherExportMock.mockReturnValue({
      errorMessage: null,
      statusMessage: null,
      result: null,
      isExporting: false,
      exportLauncher,
    });

    renderLaunchTab({ onPreviewLaunch, onLaunch });

    await user.click(screen.getByRole('button', { name: 'Dry-run' }));
    expect(onPreviewLaunch).toHaveBeenCalledWith(
      expect.objectContaining({ game_path: '/games/synthetic-quest/game.exe' })
    );

    await user.click(screen.getByRole('button', { name: 'Copy' }));
    expect(copyToClipboardMock).toHaveBeenCalledWith('gamescope -- /compat/proton run /games/synthetic-quest/game.exe');
    expect(screen.getByText('Command copied.')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: '.desktop' }));
    expect(exportLauncher).toHaveBeenCalledTimes(1);
    expect(useLauncherExportMock).toHaveBeenCalledWith(
      expect.objectContaining({
        request: expect.objectContaining({
          launcher_name: 'Synthetic Quest',
          profile_name: 'Synthetic Quest',
          trainer_path: '/trainers/synthetic-quest/trainer.exe',
        }),
      })
    );

    await user.click(screen.getByRole('button', { name: 'Launch' }));
    expect(onLaunch).toHaveBeenCalledWith('Synthetic Quest');
  });

  it('reports copy failures without throwing', async () => {
    const user = userEvent.setup();
    copyToClipboardMock.mockRejectedValue(new Error('denied'));

    renderLaunchTab();

    await user.click(screen.getByRole('button', { name: 'Copy' }));

    expect(screen.getByRole('alert')).toHaveTextContent('Failed to copy command.');
  });

  it('autosaves valid environment edits after the 400ms debounce', async () => {
    const user = userEvent.setup();
    renderLaunchTab();

    const envSection = screen.getByRole('region', { name: 'Environment' });
    const valueInput = within(envSection).getByLabelText('Value');
    await user.clear(valueInput);
    await user.type(valueInput, 'full');
    valueInput.blur();

    await waitFor(
      () => {
        expect(persistProfileDraftSpy).toHaveBeenCalledWith(
          'Synthetic Quest',
          expect.objectContaining({
            launch: expect.objectContaining({ custom_env_vars: { DXVK_HUD: 'full' } }),
          })
        );
      },
      { timeout: 1000 }
    );
  });

  it('does not autosave invalid environment rows', async () => {
    vi.useFakeTimers();
    try {
      renderLaunchTab();

      const envSection = screen.getByRole('region', { name: 'Environment' });
      const keyInput = within(envSection).getByLabelText('Key');
      fireEvent.change(keyInput, { target: { value: 'PATH' } });
      fireEvent.blur(keyInput);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(500);
      });

      expect(persistProfileDraftSpy).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });
});
