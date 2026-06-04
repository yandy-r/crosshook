import { act, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLaunchPreview, makeLaunchRequest, makeLibraryCardData, makeProfileDraft } from '@/test/fixtures';
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

// Mock HeroLaunchGate so the shell test only exercises HeroDetailLaunchTab's
// own layout: the gate section, and the hooks placeholder.
// The mock forwards onPreviewLaunch (Dry-run) so we can verify it is wired
// through from HeroDetailLaunchTab without exercising the full gate logic.
vi.mock('../launch/HeroLaunchGate', () => ({
  HeroLaunchGate: (props: {
    launchRequest: unknown;
    previewLoading: boolean;
    preview: unknown;
    previewError: string | null;
    resolvedProfileName: string;
    isLaunching: boolean;
    onPreviewLaunch?: (req: unknown) => void;
  }) => {
    const canPreview = Boolean(props.launchRequest && props.onPreviewLaunch && !props.previewLoading);
    return (
      <section aria-label="Launch gate">
        <h3>Launch command</h3>
        {!props.launchRequest ? (
          <p>Launch preview is unavailable until the game executable is set on this profile.</p>
        ) : null}
        {props.previewError ? <p role="alert">{props.previewError}</p> : null}
        <div>
          <button type="button" disabled={!canPreview} onClick={() => props.onPreviewLaunch?.(props.launchRequest)}>
            {props.previewLoading ? 'Building...' : 'Dry-run'}
          </button>
        </div>
        <div data-testid="hero-launch-subtabs-host" />
      </section>
    );
  },
  default: () => null,
}));

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
      preview={makeLaunchPreview()}
      previewError={null}
      displayProfileName="Synthetic Quest"
      {...props}
    />
  );
}

describe('HeroDetailLaunchTab', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

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
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    consoleErrorSpy.mockRestore();
  });

  it('renders the launch gate, sub-tabs host, and hooks sections', () => {
    renderLaunchTab();

    const headings = screen.getAllByRole('heading', { level: 3 }).map((heading) => heading.textContent);
    expect(headings).toContain('Launch command');
    expect(headings).toContain('Pre/post hooks');
    expect(screen.getByTestId('hero-launch-subtabs-host')).toBeInTheDocument();
    expect(screen.getByText('No pre/post hooks configured yet')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add hook (not yet available)' })).toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('disables dry-run when launch request is unavailable', () => {
    renderLaunchTab({ launchRequest: null, preview: null });

    expect(screen.getByRole('button', { name: 'Dry-run' })).toBeDisabled();
    expect(
      screen.getByText('Launch preview is unavailable until the game executable is set on this profile.')
    ).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('passes onPreviewLaunch through to the gate', async () => {
    const user = userEvent.setup();
    const onPreviewLaunch = vi.fn();

    renderLaunchTab({ onPreviewLaunch });

    await user.click(screen.getByRole('button', { name: 'Dry-run' }));
    expect(onPreviewLaunch).toHaveBeenCalledWith(
      expect.objectContaining({ game_path: '/games/synthetic-quest/game.exe' })
    );
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('does not autosave invalid environment rows', async () => {
    vi.useFakeTimers();
    try {
      renderLaunchTab();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(500);
      });

      expect(persistProfileDraftSpy).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });
});
