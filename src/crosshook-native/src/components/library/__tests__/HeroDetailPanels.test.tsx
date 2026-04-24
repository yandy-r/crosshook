import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps } from 'react';
import { describe, expect, it } from 'vitest';
import { ProfileProvider } from '@/context/ProfileContext';
import type { UseGameMetadataResult } from '@/hooks/useGameMetadata';
import { makeLibraryCardData } from '@/test/fixtures';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import { HeroDetailPanels } from '../HeroDetailPanels';

type HeroDetailPanelsProps = ComponentProps<typeof HeroDetailPanels>;

const metaStub: UseGameMetadataResult = {
  appId: '',
  state: 'idle',
  loading: false,
  result: {
    app_id: '',
    state: 'idle',
    app_details: null,
    from_cache: false,
    is_stale: false,
  },
  appDetails: null,
  fromCache: false,
  isStale: false,
  isUnavailable: false,
  refresh: async () => {},
};

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
    optimizations: {
      enabled_option_ids: ['show_mangohud_overlay'],
    },
    launch_trainer_only: false,
    launch_game_only: false,
    profile_name: 'Synthetic Quest',
    custom_env_vars: {
      PROTON_LOG: '1',
    },
    network_isolation: false,
    gamescope: {
      enabled: true,
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
      enabled: true,
      gpu_stats: true,
      cpu_stats: true,
      ram: false,
      frametime: true,
      battery: false,
      watt: false,
    },
  };
}

function makePreview(overrides: Partial<LaunchPreview> = {}): LaunchPreview {
  return {
    resolved_method: 'proton_run',
    validation: {
      issues: [
        {
          severity: 'fatal',
          message: 'Game executable is missing execute permissions.',
          help: 'Run chmod +x on the selected game binary.',
          code: 'game_executable_not_executable',
        },
        {
          severity: 'warning',
          message: 'Trainer hash differs from the cached checksum.',
          help: 'Re-download the trainer if you do not trust the file.',
          code: 'trainer_hash_changed',
        },
      ],
    },
    environment: [
      { key: 'DXVK_HUD', value: '1', source: 'profile_custom' },
      { key: 'PROTON_LOG', value: '1', source: 'profile_custom' },
      { key: 'MANGOHUD', value: '1', source: 'launch_optimization' },
      { key: 'WINEPREFIX', value: '/prefixes/synthetic-quest', source: 'proton_runtime' },
    ],
    cleared_variables: ['LD_PRELOAD', 'PRESSURE_VESSEL_FILESYSTEMS_RW'],
    wrappers: ['gamescope', 'mangohud'],
    effective_command: 'gamescope mangohud -- /usr/bin/umu-run /games/synthetic-quest/game.exe',
    directives_error: 'One launch directive could not be expanded.',
    steam_launch_options: 'PROTON_LOG=1 %command%',
    proton_setup: {
      wine_prefix_path: '/prefixes/synthetic-quest',
      compat_data_path: '/steam/compatdata/9999001',
      steam_client_install_path: '/steam/root',
      proton_executable: '/compatibilitytools/proton-ge/proton',
      umu_run_path: '/usr/bin/umu-run',
    },
    working_directory: '/games/synthetic-quest',
    game_executable: '/games/synthetic-quest/game.exe',
    game_executable_name: 'game.exe',
    trainer: {
      path: '/trainers/synthetic-quest/trainer.exe',
      host_path: '/trainers/synthetic-quest/trainer.exe',
      loading_mode: 'copy_to_prefix',
      staged_path: '/prefixes/synthetic-quest/drive_c/trainers/trainer.exe',
    },
    generated_at: '2026-04-23T12:00:00.000Z',
    display_text: 'Raw launch preview dump',
    umu_decision: null,
    ...overrides,
  };
}

function renderHeroDetailPanels(overrides: Partial<HeroDetailPanelsProps> = {}) {
  const props: HeroDetailPanelsProps = {
    mode: 'launch-options',
    summary: makeLibraryCardData(),
    steamAppId: '9999001',
    meta: metaStub,
    profile: null,
    loadState: 'idle',
    profileError: null,
    healthReport: undefined,
    healthLoading: false,
    offlineReport: undefined,
    offlineError: null,
    launchRequest: makeLaunchRequest(),
    previewLoading: false,
    preview: makePreview(),
    previewError: null,
    updateProfile: undefined,
    profileList: undefined,
    onSetActiveTab: undefined,
    ...overrides,
  };

  return render(<HeroDetailPanels {...props} />);
}

describe('HeroDetailPanels', () => {
  it('renders the structured launch preview with grouped environment output and detail sections', async () => {
    const user = userEvent.setup();
    renderHeroDetailPanels();

    expect(screen.getByRole('heading', { name: 'Summary' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Validation' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Command chain' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Proton setup' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Trainer' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Environment' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Raw preview' })).toBeInTheDocument();

    expect(screen.getByText('Proton launch')).toBeInTheDocument();
    expect(screen.getByText('gamescope -> mangohud')).toBeInTheDocument();
    expect(screen.getByText('PROTON_LOG=1 %command%')).toBeInTheDocument();
    expect(screen.getByText('One launch directive could not be expanded.')).toBeInTheDocument();

    const validationList = screen.getByRole('list', { name: 'Launch validation issues' });
    expect(within(validationList).getByText(/Error:/)).toBeInTheDocument();
    expect(within(validationList).getByText(/Game executable is missing execute permissions\./)).toBeInTheDocument();
    expect(within(validationList).getByText(/Warning:/)).toBeInTheDocument();
    expect(within(validationList).getByText(/Trainer hash differs from the cached checksum\./)).toBeInTheDocument();

    await user.click(screen.getByText('Profile custom (2)'));
    const profileCustomGroup = screen.getByText('Profile custom (2)').closest('details');
    expect(profileCustomGroup).not.toBeNull();
    expect(within(profileCustomGroup as HTMLDetailsElement).getByText(/DXVK_HUD = "1"/)).toBeInTheDocument();
    expect(within(profileCustomGroup as HTMLDetailsElement).getByText(/PROTON_LOG = "1"/)).toBeInTheDocument();

    await user.click(screen.getByText('Cleared variables (2)'));
    expect(screen.getByText(/LD_PRELOAD/)).toBeInTheDocument();
    expect(screen.getByText(/PRESSURE_VESSEL_FILESYSTEMS_RW/)).toBeInTheDocument();

    await user.click(screen.getByText('Raw preview dump'));
    expect(screen.getByText('Raw launch preview dump')).toBeInTheDocument();
  });

  it('renders structured preview empty states when optional launch preview data is absent', () => {
    renderHeroDetailPanels({
      preview: makePreview({
        validation: { issues: [] },
        environment: null,
        cleared_variables: [],
        wrappers: null,
        effective_command: null,
        directives_error: null,
        steam_launch_options: null,
        proton_setup: null,
        trainer: null,
        display_text: '',
      }),
    });

    expect(screen.getByText('All checks passed.')).toBeInTheDocument();
    expect(screen.getByText('No effective command resolved.')).toBeInTheDocument();
    expect(screen.getByText('No environment variables resolved.')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Raw preview' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Proton setup' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Trainer' })).not.toBeInTheDocument();
  });

  it('renders the launch-options empty state before a launch request is available', () => {
    renderHeroDetailPanels({
      launchRequest: null,
      preview: null,
    });

    expect(
      screen.getByText('Launch preview is unavailable until the game executable is set on this profile.')
    ).toBeInTheDocument();
  });
});

describe('no-op defaults', () => {
  it('renders read-only panels when updateProfile is omitted', () => {
    render(
      <ProfileProvider>
        <HeroDetailPanels
          mode="profiles"
          summary={makeLibraryCardData()}
          steamAppId="9999001"
          meta={metaStub}
          profile={null}
          loadState="idle"
          profileError={null}
          healthReport={undefined}
          healthLoading={false}
          offlineReport={undefined}
          offlineError={null}
          launchRequest={null}
          previewLoading={false}
          preview={null}
          previewError={null}
          updateProfile={undefined}
          profileList={undefined}
          onSetActiveTab={undefined}
        />
      </ProfileProvider>
    );

    expect(screen.getByRole('heading', { name: 'Active profile' })).toBeInTheDocument();
    expect(screen.getByText('No active profile loaded in the editor for this game.')).toBeInTheDocument();
  });
});
