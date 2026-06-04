/**
 * Focused tests for HeroLaunchSubTabsHost.
 *
 * Tests:
 *  - LaunchSubTabs is rendered with correct isInsideGamescopeSession prop
 *  - profileMismatch=true renders disabled overlay with hint text,
 *    passes aria-disabled to the wrapper, and unmounts LaunchSubTabs
 *  - profileMismatch=false renders without the overlay
 *
 * Strategy A: hand-rolled vi.mock of ProfileContext, LaunchSubTabs, and
 * useLaunchSubTabsProps.
 */
import { render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '@/types/profile';
import { HeroLaunchSubTabsHost } from '../launch/HeroLaunchSubTabsHost';

const profileContextMock = vi.fn();

// Stub ProfileContext — HeroLaunchSubTabsHost reads `profileName` from it for
// the mismatch overlay text.
vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

// Stub useLaunchSubTabsProps so we don't need the full provider stack.
// Capture the props it was called with so we can assert them.
const useLaunchSubTabsPropsMock = vi.fn();
vi.mock('@/hooks/launch/useLaunchSubTabsProps', () => ({
  useLaunchSubTabsProps: (input: unknown) => useLaunchSubTabsPropsMock(input),
}));

// Stub LaunchSubTabs — just needs to be present in the DOM.
vi.mock('@/components/LaunchSubTabs', () => ({
  LaunchSubTabs: (props: { launchMethod: string }) => (
    <div data-testid="launch-subtabs" data-method={props.launchMethod} />
  ),
}));

const baseSubTabsProps = {
  launchMethod: 'proton_run' as const,
  steamAppId: '9999001',
  gamescopeConfig: DEFAULT_GAMESCOPE_CONFIG,
  onGamescopeChange: vi.fn(),
  isInsideGamescopeSession: false,
  mangoHudConfig: DEFAULT_MANGOHUD_CONFIG,
  onMangoHudChange: vi.fn(),
  showMangoHudOverlayEnabled: false,
  enabledOptionIds: [] as string[],
  onToggleOption: vi.fn(),
  launchOptimizationsStatus: { tone: 'idle' as const, label: '' },
  catalog: null,
  profileName: 'Synthetic Quest',
  onUpdateProfile: vi.fn(),
  showProtonDbLookup: true,
  onApplyProtonDbEnvVars: vi.fn(),
  applyingProtonDbGroupId: null,
  protonDbStatusMessage: null,
  pendingProtonDbOverwrite: null,
  onConfirmProtonDbOverwrite: vi.fn(),
  onCancelProtonDbOverwrite: vi.fn(),
  onUpdateProtonDbResolution: vi.fn(),
  gamescopeAutoSaveStatus: { tone: 'idle' as const, label: '' },
  mangoHudAutoSaveStatus: { tone: 'idle' as const, label: '' },
};

function renderSubTabsHost(props: Partial<React.ComponentProps<typeof HeroLaunchSubTabsHost>> = {}) {
  profileContextMock.mockReturnValue({
    profile: makeProfileDraft(),
    profileName: 'Synthetic Quest',
    selectedProfile: 'Synthetic Quest',
    profiles: ['Synthetic Quest'],
  });
  useLaunchSubTabsPropsMock.mockReturnValue({ ...baseSubTabsProps });

  return render(
    <HeroLaunchSubTabsHost
      resolvedProfileName="Synthetic Quest"
      resolvedSteamAppId="9999001"
      hasSavedSelectedProfile={true}
      profileMismatch={false}
      {...props}
    />
  );
}

describe('HeroLaunchSubTabsHost', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
  });

  // ── Normal rendering ─────────────────────────────────────────────────────────

  it('renders LaunchSubTabs without a mismatch overlay', () => {
    renderSubTabsHost({ profileMismatch: false });
    expect(screen.getByTestId('launch-subtabs')).toBeInTheDocument();
    expect(screen.queryByRole('listitem')).not.toBeInTheDocument();
    // No overlay paragraph
    expect(screen.queryByText(/apply to the selected profile/i)).not.toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('passes isGamescopeRunning=false to useLaunchSubTabsProps by default', () => {
    renderSubTabsHost();
    expect(useLaunchSubTabsPropsMock).toHaveBeenCalledWith(expect.objectContaining({ isGamescopeRunning: false }));
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('passes isGamescopeRunning=true to useLaunchSubTabsProps when provided', () => {
    renderSubTabsHost({ isGamescopeRunning: true });
    expect(useLaunchSubTabsPropsMock).toHaveBeenCalledWith(expect.objectContaining({ isGamescopeRunning: true }));
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('passes resolvedSteamAppId and hasSavedSelectedProfile to useLaunchSubTabsProps', () => {
    renderSubTabsHost({ resolvedSteamAppId: '12345', hasSavedSelectedProfile: false });
    expect(useLaunchSubTabsPropsMock).toHaveBeenCalledWith(
      expect.objectContaining({ resolvedSteamAppId: '12345', hasSavedSelectedProfile: false })
    );
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── Profile mismatch ─────────────────────────────────────────────────────────

  it('renders the mismatch overlay when profileMismatch=true', () => {
    renderSubTabsHost({ profileMismatch: true });
    expect(screen.getByText(/Launch settings apply to the selected profile/i)).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('unmounts LaunchSubTabs when profileMismatch=true', () => {
    renderSubTabsHost({ profileMismatch: true });
    expect(screen.queryByTestId('launch-subtabs')).not.toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('wrapper has aria-disabled when profileMismatch=true', () => {
    const { container } = renderSubTabsHost({ profileMismatch: true });
    const wrapper = container.querySelector('.crosshook-hero-detail__subtabs-host');
    expect(wrapper).toHaveAttribute('aria-disabled', 'true');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('wrapper does NOT have aria-disabled when profileMismatch=false', () => {
    const { container } = renderSubTabsHost({ profileMismatch: false });
    const wrapper = container.querySelector('.crosshook-hero-detail__subtabs-host');
    expect(wrapper).not.toHaveAttribute('aria-disabled');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('mismatch overlay shows the selected profile name from ProfileContext', () => {
    profileContextMock.mockReturnValue({
      profile: makeProfileDraft(),
      profileName: 'Other Profile',
      selectedProfile: 'Other Profile',
      profiles: ['Other Profile'],
    });
    useLaunchSubTabsPropsMock.mockReturnValue({ ...baseSubTabsProps });

    render(
      <HeroLaunchSubTabsHost
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={true}
        profileMismatch={true}
      />
    );

    expect(screen.getByText(/Other Profile/)).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('wrapper has mismatch modifier class when profileMismatch=true', () => {
    const { container } = renderSubTabsHost({ profileMismatch: true });
    const wrapper = container.querySelector('.crosshook-hero-detail__subtabs-host');
    expect(wrapper?.className).toContain('crosshook-hero-detail__subtabs-host--mismatch');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('wrapper does NOT have mismatch modifier class when profileMismatch=false', () => {
    const { container } = renderSubTabsHost({ profileMismatch: false });
    const wrapper = container.querySelector('.crosshook-hero-detail__subtabs-host');
    expect(wrapper?.className).not.toContain('--mismatch');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
