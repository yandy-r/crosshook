import { render, screen } from '@testing-library/react';
import type { ComponentProps } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import type { UseGameMetadataResult } from '@/hooks/useGameMetadata';
import { makeLaunchPreview, makeLaunchRequest, makeLibraryCardData } from '@/test/fixtures';
import type { LaunchPreview, LaunchRequest } from '@/types/launch';
import type { HeroDetailLaunchTabProps } from '../HeroDetailLaunchTab';
import { HeroDetailPanels } from '../HeroDetailPanels';

vi.mock('../HeroDetailLaunchTab', () => ({
  HeroDetailLaunchTab: ({ launchRequest, previewLoading, preview, previewError }: HeroDetailLaunchTabProps) => (
    <div data-testid="mock-hero-detail-launch-tab" className="crosshook-hero-detail__launch-tab">
      <section aria-label="Launch command">
        <h3>Launch command</h3>
        {!launchRequest ? <p>Launch preview is unavailable until the game executable is set on this profile.</p> : null}
        {previewLoading ? <p>Building launch preview...</p> : null}
        {previewError ? <p>{previewError}</p> : null}
        {preview ? <pre>{preview.effective_command}</pre> : null}
      </section>
      <section aria-label="Environment">
        <h3>Environment</h3>
      </section>
      <section aria-label="Pre/post hooks">
        <h3>Pre/post hooks</h3>
        <p>No pre/post hooks configured yet</p>
      </section>
    </div>
  ),
}));

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

// Local flavor over the canonical builders: this suite exercises a richer
// preview (validation issues, wrappers, directives error, staged trainer).
function makePanelsLaunchRequest(): LaunchRequest {
  return makeLaunchRequest({
    optimizations: {
      enabled_option_ids: ['show_mangohud_overlay'],
    },
    custom_env_vars: {
      PROTON_LOG: '1',
    },
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
    mangohud: {
      enabled: true,
      gpu_stats: true,
      cpu_stats: true,
      ram: false,
      frametime: true,
      battery: false,
      watt: false,
    },
  });
}

function makePreview(overrides: Partial<LaunchPreview> = {}): LaunchPreview {
  return makeLaunchPreview({
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
    trainer: {
      path: '/trainers/synthetic-quest/trainer.exe',
      host_path: '/trainers/synthetic-quest/trainer.exe',
      loading_mode: 'copy_to_prefix',
      staged_path: '/prefixes/synthetic-quest/drive_c/trainers/trainer.exe',
    },
    display_text: 'Raw launch preview dump',
    ...overrides,
  });
}

function renderHeroDetailPanels(
  overrides: Partial<HeroDetailPanelsProps> = {},
  options: { withProviders?: boolean } = {}
) {
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
    launchRequest: makePanelsLaunchRequest(),
    previewLoading: false,
    preview: makePreview(),
    previewError: null,
    updateProfile: undefined,
    profileList: undefined,
    onSetActiveTab: undefined,
    ...overrides,
  };

  const panel = <HeroDetailPanels {...props} />;

  if (options.withProviders) {
    return render(
      <PreferencesProvider>
        <ProfileProvider>
          <ProfileHealthProvider>{panel}</ProfileHealthProvider>
        </ProfileProvider>
      </PreferencesProvider>
    );
  }

  return render(panel);
}

describe('HeroDetailPanels', () => {
  it('renders the launch-options branch as the new three-section launch tab', () => {
    renderHeroDetailPanels();

    expect(screen.getByTestId('mock-hero-detail-launch-tab')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Launch command' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Environment' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Pre/post hooks' })).toBeInTheDocument();
    expect(
      screen.getByText('gamescope mangohud -- /usr/bin/umu-run /games/synthetic-quest/game.exe')
    ).toBeInTheDocument();
    expect(screen.getByText('No pre/post hooks configured yet')).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Summary' })).not.toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: 'Raw preview' })).not.toBeInTheDocument();
  });

  it('renders launch tab loading and error state through the launch-options branch', () => {
    renderHeroDetailPanels({
      previewLoading: true,
      previewError: 'Preview failed.',
      preview: null,
    });

    expect(screen.getByText('Building launch preview...')).toBeInTheDocument();
    expect(screen.getByText('Preview failed.')).toBeInTheDocument();
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
  it('renders the profiles tab empty state when updateProfile is omitted', () => {
    renderHeroDetailPanels(
      { mode: 'profiles', launchRequest: null, preview: null, profileList: [] },
      { withProviders: true }
    );

    expect(screen.getByRole('complementary', { name: 'Profiles for this game' })).toBeInTheDocument();
    expect(screen.getByText('No profiles found for this game.')).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Profile editor' })).toBeInTheDocument();
  });
});
