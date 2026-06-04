/**
 * Focused tests for HeroProfileEditorSections and its sub-panels:
 *
 * - Trainer-gamescope panel: derived-from-game notice renders per the
 *   resolveTrainerGamescopeForDisplay logic; onChange routes through
 *   onUpdateProfile (full-draft), NOT granular IPC.
 * - Trainer section edit + trainer version set field visibility.
 * - Prefix-deps panel renders inside CollapsibleSection when deps required;
 *   absent otherwise.
 * - Runtime suggestion banner: suggestion shown, install CTA, dismiss.
 * - Health issues list renders + badge click scrolls to ref +
 *   stale-check note.
 * - Full LauncherExport slot mounts in the export slot when passed.
 *
 * Strategy A: hand-rolled vi.mock of contexts/IPC; child sections that are
 * not under test are stubbed to <div>; the tested sub-components render real.
 */

import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ComponentProps, RefObject } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import type { EnrichedProfileHealthReport, HealthIssue } from '@/types/health';
import type { GamescopeConfig, LaunchMethod } from '@/types/profile';
import { DEFAULT_GAMESCOPE_CONFIG } from '@/types/profile';
import type { ProtonUpSuggestion } from '@/types/protonup';
import { HeroProfileEditorSections } from '../profiles/HeroProfileEditorSections';

// ---------------------------------------------------------------------------
// Stub heavy sections that are not under test
// ---------------------------------------------------------------------------

vi.mock('../../profile-sections/ProfileIdentitySection', () => ({
  ProfileIdentitySection: () => <div>Identity Section</div>,
}));

vi.mock('../../profile-sections/RunnerMethodSection', () => ({
  RunnerMethodSection: () => <div>Runner Method Section</div>,
}));

vi.mock('../../profile-sections/RuntimeSection', () => ({
  RuntimeSection: () => <div>Runtime Section</div>,
}));

vi.mock('../../profile-sections/GameSection', () => ({
  GameSection: () => <div>Game Section</div>,
}));

vi.mock('../../profile-sections/MediaSection', () => ({
  MediaSection: () => <div>Media Section</div>,
}));

vi.mock('../../profile-sections/GameMetadataBar', () => ({
  GameMetadataBar: () => null,
}));

vi.mock('../../profile-sections/TrainerSection', () => ({
  TrainerSection: ({ trainerVersion }: { trainerVersion?: string | null }) => (
    <div data-testid="trainer-section">
      {trainerVersion != null ? <span data-testid="trainer-version">{trainerVersion}</span> : null}
    </div>
  ),
}));

vi.mock('../../GamescopeConfigPanel', () => ({
  GamescopeConfigPanel: ({
    config,
    onChange,
    derivedConfigNotice,
  }: {
    config: GamescopeConfig;
    onChange: (c: GamescopeConfig) => void;
    derivedConfigNotice?: string;
  }) => (
    <div data-testid="gamescope-config-panel">
      {derivedConfigNotice ? <p data-testid="derived-config-notice">{derivedConfigNotice}</p> : null}
      <button
        type="button"
        data-testid="gamescope-trigger-change"
        onClick={() => onChange({ ...config, enabled: !config.enabled })}
      >
        Toggle gamescope
      </button>
    </div>
  ),
  default: ({
    config,
    onChange,
    derivedConfigNotice,
  }: {
    config: GamescopeConfig;
    onChange: (c: GamescopeConfig) => void;
    derivedConfigNotice?: string;
  }) => (
    <div data-testid="gamescope-config-panel">
      {derivedConfigNotice ? <p data-testid="derived-config-notice">{derivedConfigNotice}</p> : null}
      <button
        type="button"
        data-testid="gamescope-trigger-change"
        onClick={() => onChange({ ...config, enabled: !config.enabled })}
      >
        Toggle gamescope
      </button>
    </div>
  ),
}));

vi.mock('../../PrefixDepsPanel', () => ({
  PrefixDepsPanel: ({ requiredPackages }: { requiredPackages: string[] }) => (
    <div data-testid="prefix-deps-panel">{requiredPackages.join(', ')}</div>
  ),
  default: ({ requiredPackages }: { requiredPackages: string[] }) => (
    <div data-testid="prefix-deps-panel">{requiredPackages.join(', ')}</div>
  ),
}));

vi.mock('../../HealthBadge', () => ({
  HealthBadge: ({ onClick }: { onClick?: () => void }) => (
    <button type="button" data-testid="health-badge" onClick={onClick}>
      Health Badge
    </button>
  ),
  default: ({ onClick }: { onClick?: () => void }) => (
    <button type="button" data-testid="health-badge" onClick={onClick}>
      Health Badge
    </button>
  ),
}));

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

function makeHealthIssue(overrides: Partial<HealthIssue> = {}): HealthIssue {
  return {
    field: 'executable_path',
    path: '/games/game.exe',
    message: 'File not found.',
    remediation: 'Ensure the path is correct.',
    severity: 'error',
    ...overrides,
  };
}

function makeEnrichedReport(overrides: Partial<EnrichedProfileHealthReport> = {}): EnrichedProfileHealthReport {
  return {
    name: 'my-profile',
    status: 'broken',
    launch_method: 'proton_run',
    issues: [makeHealthIssue()],
    checked_at: '2025-01-01T00:00:00.000Z',
    metadata: null,
    ...overrides,
  };
}

type SectionsProps = ComponentProps<typeof HeroProfileEditorSections>;

function buildBaseProps(overrides: Partial<SectionsProps> = {}): SectionsProps {
  return {
    profile: makeProfileDraft(),
    profileName: 'my-profile',
    profileExists: true,
    profiles: ['my-profile'],
    launchMethod: 'proton_run' as LaunchMethod,
    protonInstalls: [],
    protonInstallsError: null,
    onUpdateProfile: vi.fn(),
    onProfileNameChange: vi.fn(),
    ...overrides,
  };
}

let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  vi.clearAllMocks();
  consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
  expect(consoleErrorSpy).not.toHaveBeenCalled();
  consoleErrorSpy.mockRestore();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('HeroProfileEditorSections', () => {
  describe('Trainer section visibility', () => {
    it('renders TrainerSection for proton_run launch method', () => {
      render(<HeroProfileEditorSections {...buildBaseProps({ launchMethod: 'proton_run' })} />);

      expect(screen.getByTestId('trainer-section')).toBeInTheDocument();
    });

    it('does not render TrainerSection for native launch method', () => {
      render(<HeroProfileEditorSections {...buildBaseProps({ launchMethod: 'native' as LaunchMethod })} />);

      expect(screen.queryByTestId('trainer-section')).not.toBeInTheDocument();
    });

    it('passes trainerVersion to TrainerSection when provided', () => {
      render(
        <HeroProfileEditorSections {...buildBaseProps({ trainerVersion: '1.42.0', launchMethod: 'proton_run' })} />
      );

      expect(screen.getByTestId('trainer-version')).toHaveTextContent('1.42.0');
    });
  });

  describe('Trainer-Gamescope panel', () => {
    it('renders GamescopeConfigPanel when launch method is not native', () => {
      render(<HeroProfileEditorSections {...buildBaseProps()} />);

      expect(screen.getByTestId('gamescope-config-panel')).toBeInTheDocument();
    });

    it('hides GamescopeConfigPanel for native launch method', () => {
      render(<HeroProfileEditorSections {...buildBaseProps({ launchMethod: 'native' as LaunchMethod })} />);

      expect(screen.queryByTestId('gamescope-config-panel')).not.toBeInTheDocument();
    });

    it('shows derived-from-game notice when game gamescope is enabled but trainer_gamescope is not', () => {
      const profile = makeProfileDraft({
        launch: {
          method: 'proton_run' as LaunchMethod,
          optimizations: { enabled_option_ids: [] },
          custom_env_vars: {},
          gamescope: {
            ...DEFAULT_GAMESCOPE_CONFIG,
            enabled: true,
          },
          trainer_gamescope: {
            ...DEFAULT_GAMESCOPE_CONFIG,
            enabled: false,
          },
        },
      });

      render(<HeroProfileEditorSections {...buildBaseProps({ profile })} />);

      expect(screen.getByTestId('derived-config-notice')).toHaveTextContent(/auto-generated from the game config/i);
    });

    it('does not show derived-from-game notice when trainer_gamescope is independently enabled', () => {
      const profile = makeProfileDraft({
        launch: {
          method: 'proton_run' as LaunchMethod,
          optimizations: { enabled_option_ids: [] },
          custom_env_vars: {},
          gamescope: {
            ...DEFAULT_GAMESCOPE_CONFIG,
            enabled: true,
          },
          trainer_gamescope: {
            ...DEFAULT_GAMESCOPE_CONFIG,
            enabled: true,
          },
        },
      });

      render(<HeroProfileEditorSections {...buildBaseProps({ profile })} />);

      expect(screen.queryByTestId('derived-config-notice')).not.toBeInTheDocument();
    });

    it('routes gamescope changes through onUpdateProfile (not granular IPC)', async () => {
      const user = userEvent.setup();
      const onUpdateProfile = vi.fn();

      render(<HeroProfileEditorSections {...buildBaseProps({ onUpdateProfile })} />);

      await user.click(screen.getByTestId('gamescope-trigger-change'));

      expect(onUpdateProfile).toHaveBeenCalledOnce();
      // Verify the updater sets trainer_gamescope on the launch section
      const updater = onUpdateProfile.mock.calls[0][0] as (
        p: ReturnType<typeof makeProfileDraft>
      ) => ReturnType<typeof makeProfileDraft>;
      const original = makeProfileDraft();
      const updated = updater(original);
      expect(updated.launch).toHaveProperty('trainer_gamescope');
    });
  });

  describe('Prefix dependencies panel', () => {
    it('renders PrefixDepsPanel inside CollapsibleSection when required_protontricks is non-empty', () => {
      const profile = makeProfileDraft({
        trainer: {
          path: '/trainers/game/trainer.exe',
          type: '',
          trainer_type: 'unknown',
          loading_mode: 'source_directory',
          required_protontricks: ['vcrun2019', 'd3dx11_43'],
        },
      });

      render(<HeroProfileEditorSections {...buildBaseProps({ profile })} />);

      const panel = screen.getByTestId('prefix-deps-panel');
      expect(panel).toBeInTheDocument();
      expect(panel).toHaveTextContent('vcrun2019');
      expect(panel).toHaveTextContent('d3dx11_43');
    });

    it('does not render PrefixDepsPanel when required_protontricks is empty', () => {
      const profile = makeProfileDraft({
        trainer: {
          path: '/trainers/game/trainer.exe',
          type: '',
          trainer_type: 'unknown',
          loading_mode: 'source_directory',
          required_protontricks: [],
        },
      });

      render(<HeroProfileEditorSections {...buildBaseProps({ profile })} />);

      expect(screen.queryByTestId('prefix-deps-panel')).not.toBeInTheDocument();
    });

    it('does not render PrefixDepsPanel when required_protontricks is absent', () => {
      const profile = makeProfileDraft({
        trainer: {
          path: '',
          type: '',
          trainer_type: 'unknown',
          loading_mode: 'source_directory',
        },
      });

      render(<HeroProfileEditorSections {...buildBaseProps({ profile })} />);

      expect(screen.queryByTestId('prefix-deps-panel')).not.toBeInTheDocument();
    });
  });

  describe('Runtime suggestion banner', () => {
    it('renders suggestion banner when suggestion status is missing and not dismissed', () => {
      const suggestion: ProtonUpSuggestion = {
        status: 'missing',
        community_version: 'GE-Proton9-1',
        recommended_version: 'GE-Proton9-1',
      };

      render(
        <HeroProfileEditorSections
          {...buildBaseProps({
            suggestion,
            suggestionDismissed: false,
            effectiveSteamClientInstallPath: '/home/user/.steam/steam',
          })}
        />
      );

      expect(screen.getByRole('status')).toBeInTheDocument();
      expect(screen.getByText(/runtime suggestion/i)).toBeInTheDocument();
      expect(screen.getByText(/GE-Proton9-1/)).toBeInTheDocument();
    });

    it('hides suggestion banner when suggestionDismissed is true', () => {
      const suggestion: ProtonUpSuggestion = {
        status: 'missing',
        community_version: 'GE-Proton9-1',
        recommended_version: 'GE-Proton9-1',
      };

      render(<HeroProfileEditorSections {...buildBaseProps({ suggestion, suggestionDismissed: true })} />);

      expect(screen.queryByRole('status')).not.toBeInTheDocument();
    });

    it('calls onInstallSuggestedVersion when install CTA is clicked', async () => {
      const user = userEvent.setup();
      const onInstallSuggestedVersion = vi.fn();
      const suggestion: ProtonUpSuggestion = {
        status: 'missing',
        community_version: 'GE-Proton9-1',
        recommended_version: 'GE-Proton9-1',
      };

      render(
        <HeroProfileEditorSections
          {...buildBaseProps({
            suggestion,
            suggestionDismissed: false,
            effectiveSteamClientInstallPath: '/home/user/.steam/steam',
            onInstallSuggestedVersion,
          })}
        />
      );

      await user.click(screen.getByRole('button', { name: /install recommended/i }));

      expect(onInstallSuggestedVersion).toHaveBeenCalled();
    });

    it('calls onDismissSuggestion when Dismiss is clicked', async () => {
      const user = userEvent.setup();
      const onDismissSuggestion = vi.fn();
      const suggestion: ProtonUpSuggestion = {
        status: 'missing',
        community_version: 'GE-Proton9-1',
        recommended_version: 'GE-Proton9-1',
      };

      render(
        <HeroProfileEditorSections
          {...buildBaseProps({
            suggestion,
            suggestionDismissed: false,
            effectiveSteamClientInstallPath: '/home/user/.steam/steam',
            onDismissSuggestion,
          })}
        />
      );

      await user.click(screen.getByRole('button', { name: /dismiss/i }));

      expect(onDismissSuggestion).toHaveBeenCalled();
    });

    it('shows suggestion install error with role="alert"', () => {
      const suggestion: ProtonUpSuggestion = {
        status: 'missing',
        community_version: 'GE-Proton9-1',
        recommended_version: 'GE-Proton9-1',
      };

      render(
        <HeroProfileEditorSections
          {...buildBaseProps({
            suggestion,
            suggestionDismissed: false,
            effectiveSteamClientInstallPath: '/home/user/.steam/steam',
            suggestionInstallError: 'ProtonUp installation failed: disk full.',
          })}
        />
      );

      expect(screen.getByRole('alert')).toHaveTextContent(/disk full/i);
    });
  });

  describe('Health issues list', () => {
    it('renders health issues inside CollapsibleSection when status is broken and issues exist', () => {
      const report = makeEnrichedReport({
        status: 'broken',
        issues: [makeHealthIssue({ field: 'executable_path', message: 'File not found.' })],
      });

      render(<HeroProfileEditorSections {...buildBaseProps({ selectedReport: report })} />);

      expect(screen.getByText(/health issues/i)).toBeInTheDocument();
      expect(screen.getByText(/file not found/i)).toBeInTheDocument();
    });

    it('does not render health issues list when status is healthy', () => {
      const report = makeEnrichedReport({
        status: 'healthy',
        issues: [],
      });

      render(<HeroProfileEditorSections {...buildBaseProps({ selectedReport: report })} />);

      expect(screen.queryByText(/health issues/i)).not.toBeInTheDocument();
    });

    it('badge click invokes scrollIntoView on the healthIssuesRef', async () => {
      const user = userEvent.setup();
      // happy-dom makes scrollIntoView a no-op but it exists on the prototype;
      // spy on it so we can assert the call without reassigning the read-only prop.
      const scrollSpy = vi.spyOn(window.HTMLElement.prototype, 'scrollIntoView').mockImplementation(() => {});

      const report = makeEnrichedReport();
      const healthIssuesRef: RefObject<HTMLDivElement> = { current: null };

      render(<HeroProfileEditorSections {...buildBaseProps({ selectedReport: report, healthIssuesRef })} />);

      await user.click(screen.getByTestId('health-badge'));

      expect(scrollSpy).toHaveBeenCalledWith({ behavior: 'smooth', block: 'start' });

      scrollSpy.mockRestore();
    });

    it('renders stale-check note when staleInfo.isStale is true and no live report', () => {
      render(
        <HeroProfileEditorSections
          {...buildBaseProps({
            selectedReport: undefined,
            selectedCachedSnapshot: {
              profile_id: 'profile-001',
              profile_name: 'my-profile',
              status: 'stale',
              issue_count: 0,
              checked_at: '2025-01-01T00:00:00.000Z',
            },
            staleInfo: { isStale: true, daysAgo: 7 },
          })}
        />
      );

      expect(screen.getByRole('note')).toHaveTextContent(/checked 7d ago/i);
    });
  });

  describe('LauncherExport slot', () => {
    it('renders the launcherExportSlot when provided', () => {
      render(
        <HeroProfileEditorSections
          {...buildBaseProps({
            launcherExportSlot: <div data-testid="launcher-export-slot">Launcher Export</div>,
          })}
        />
      );

      expect(screen.getByTestId('launcher-export-slot')).toBeInTheDocument();
    });

    it('does not render anything in the export slot when launcherExportSlot is absent', () => {
      render(<HeroProfileEditorSections {...buildBaseProps()} />);

      expect(screen.queryByTestId('launcher-export-slot')).not.toBeInTheDocument();
    });
  });
});
