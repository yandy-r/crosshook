/**
 * Focused tests for HeroLaunchCommandSection.
 *
 * Tests the command block, copy/dry-run/launch buttons, in-place mode vs legacy
 * mode, notSelectableHint, and copy status transitions.
 *
 * Strategy A: hand-rolled vi.mock of ProfileContext and PreferencesContext.
 */
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLaunchPreview, makeLaunchRequest, makeProfileDraft } from '@/test/fixtures';
import { LaunchPhase } from '@/types/launch';
import { HeroLaunchCommandSection } from '../launch/HeroLaunchCommandSection';

const profileContextMock = vi.fn();
const preferencesContextMock = vi.fn();
const useLauncherExportMock = vi.fn();
const copyToClipboardMock = vi.fn();

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

vi.mock('@/context/PreferencesContext', () => ({
  usePreferencesContext: () => preferencesContextMock(),
}));

vi.mock('@/hooks/useLauncherExport', () => ({
  useLauncherExport: (opts: unknown) => useLauncherExportMock(opts),
}));

vi.mock('@/utils/clipboard', () => ({
  copyToClipboard: (text: string) => copyToClipboardMock(text),
}));

// HighlightedCommandBlock uses heavy Shiki-based rendering — stub it out.
vi.mock('@/components/library/HighlightedCommandBlock', () => ({
  HighlightedCommandBlock: ({ preview }: { preview: { effective_command: string | null } }) => (
    <pre data-testid="command-block">{preview.effective_command}</pre>
  ),
}));

// DashboardPanelSection renders children + an optional actions slot.
vi.mock('@/components/layout/DashboardPanelSection', () => ({
  DashboardPanelSection: ({
    title,
    children,
    actions,
  }: {
    title: string;
    children: React.ReactNode;
    actions?: React.ReactNode;
  }) => (
    <section>
      <h3>{title}</h3>
      {actions}
      {children}
    </section>
  ),
}));

function makeProfileContext(overrides: Record<string, unknown> = {}) {
  return {
    profile: makeProfileDraft({
      game: { name: 'Synthetic Quest', executable_path: '/games/synthetic-quest/game.exe' },
      trainer: { path: '/trainers/synthetic-quest/trainer.exe', type: 'dll', loading_mode: 'source_directory' },
      runtime: { prefix_path: '/prefixes/synthetic-quest', proton_path: '/compat/proton', working_directory: '' },
      launch: {
        method: 'proton_run',
        optimizations: { enabled_option_ids: [] },
        custom_env_vars: { DXVK_HUD: 'fps' },
      },
    }),
    profileName: 'Synthetic Quest',
    selectedProfile: 'Synthetic Quest',
    profiles: ['Synthetic Quest'],
    steamClientInstallPath: '/steam/root',
    targetHomePath: '/home/devuser',
    ...overrides,
  };
}

function renderCommandSection(props: Partial<React.ComponentProps<typeof HeroLaunchCommandSection>> = {}) {
  profileContextMock.mockReturnValue(makeProfileContext());
  preferencesContextMock.mockReturnValue({ settings: { umu_preference: 'auto' } });
  useLauncherExportMock.mockReturnValue({
    errorMessage: null,
    statusMessage: null,
    result: null,
    isExporting: false,
    exportLauncher: vi.fn(),
  });

  return render(
    <HeroLaunchCommandSection
      launchRequest={makeLaunchRequest()}
      previewLoading={false}
      preview={makeLaunchPreview()}
      previewError={null}
      resolvedProfileName="Synthetic Quest"
      isLaunching={false}
      {...props}
    />
  );
}

describe('HeroLaunchCommandSection', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    copyToClipboardMock.mockResolvedValue(undefined);
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    consoleErrorSpy.mockRestore();
  });

  // ── Rendering ────────────────────────────────────────────────────────────────

  it('renders the launch command heading', () => {
    renderCommandSection();
    expect(screen.getByRole('heading', { level: 3 })).toHaveTextContent('Launch command');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders the effective_command inside the command block', () => {
    renderCommandSection();
    expect(screen.getByTestId('command-block')).toHaveTextContent(
      'gamescope -- /compat/proton run /games/synthetic-quest/game.exe'
    );
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows "no preview" message when launchRequest is null', () => {
    renderCommandSection({ launchRequest: null, preview: null });
    expect(
      screen.getByText('Launch preview is unavailable until the game executable is set on this profile.')
    ).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows building message while previewLoading', () => {
    renderCommandSection({ previewLoading: true });
    expect(screen.getByText('Building launch preview...')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows previewError as a warning paragraph', () => {
    renderCommandSection({ previewError: 'Preview build failed' });
    expect(screen.getByText('Preview build failed')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows notSelectableHint as a note when provided', () => {
    renderCommandSection({
      notSelectableHint: 'This profile is not saved and cannot be launched from the library.',
    });
    const hint = screen.getByRole('note');
    expect(hint).toHaveTextContent('This profile is not saved and cannot be launched from the library.');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── Dry-run button ───────────────────────────────────────────────────────────

  it('dry-run button is enabled when launchRequest + onPreviewLaunch are present', () => {
    renderCommandSection({ onPreviewLaunch: vi.fn() });
    expect(screen.getByRole('button', { name: 'Dry-run' })).not.toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('dry-run button is disabled when launchRequest is null', () => {
    renderCommandSection({ launchRequest: null, preview: null });
    expect(screen.getByRole('button', { name: 'Dry-run' })).toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('dry-run button shows "Building..." during previewLoading', () => {
    renderCommandSection({ previewLoading: true });
    expect(screen.getByRole('button', { name: 'Building...' })).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('calls onPreviewLaunch with the current launchRequest when dry-run clicked', async () => {
    const user = userEvent.setup();
    const onPreviewLaunch = vi.fn();
    const req = makeLaunchRequest({ profile_name: 'click-test' });
    renderCommandSection({ launchRequest: req, onPreviewLaunch });

    await user.click(screen.getByRole('button', { name: 'Dry-run' }));
    expect(onPreviewLaunch).toHaveBeenCalledWith(expect.objectContaining({ profile_name: 'click-test' }));
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── Copy button ──────────────────────────────────────────────────────────────

  it('copy button is enabled when preview has effective_command', () => {
    renderCommandSection();
    expect(screen.getByRole('button', { name: 'Copy' })).not.toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('copy button is disabled when preview is null', () => {
    renderCommandSection({ preview: null });
    expect(screen.getByRole('button', { name: 'Copy' })).toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('copy button is disabled when effective_command is null', () => {
    renderCommandSection({ preview: makeLaunchPreview({ effective_command: null }) });
    expect(screen.getByRole('button', { name: 'Copy' })).toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('disables desktop export when the displayed profile does not match the selected profile', () => {
    renderCommandSection({ canExportDesktop: false });

    expect(screen.getByRole('button', { name: '.desktop' })).toBeDisabled();
    expect(useLauncherExportMock).not.toHaveBeenCalled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows "Command copied." status after successful copy', async () => {
    const user = userEvent.setup();
    copyToClipboardMock.mockResolvedValue(undefined);
    renderCommandSection();

    await user.click(screen.getByRole('button', { name: 'Copy' }));
    await waitFor(() => {
      expect(screen.getByRole('status')).toHaveTextContent('Command copied.');
    });
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows "Failed to copy command." alert when copy throws', async () => {
    const user = userEvent.setup();
    copyToClipboardMock.mockRejectedValue(new Error('clipboard blocked'));
    renderCommandSection();

    await user.click(screen.getByRole('button', { name: 'Copy' }));
    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('Failed to copy command.');
    });
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── Legacy launch button ─────────────────────────────────────────────────────

  it('renders a single "Launch" button in legacy mode (onLaunch prop)', () => {
    renderCommandSection({ onLaunch: vi.fn() });
    expect(screen.getByRole('button', { name: 'Launch' })).toBeInTheDocument();
    // In-place buttons should not be present.
    expect(screen.queryByRole('button', { name: /Launch Game/i })).not.toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('legacy Launch button calls onLaunch with resolvedProfileName when clicked', async () => {
    const user = userEvent.setup();
    const onLaunch = vi.fn();
    renderCommandSection({ onLaunch, resolvedProfileName: 'My Profile' });

    await user.click(screen.getByRole('button', { name: 'Launch' }));
    expect(onLaunch).toHaveBeenCalledWith('My Profile');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('legacy Launch button shows "Launching..." and is disabled when isLaunching=true', () => {
    renderCommandSection({ onLaunch: vi.fn(), isLaunching: true });
    const btn = screen.getByRole('button', { name: 'Launching...' });
    expect(btn).toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── In-place launch buttons ──────────────────────────────────────────────────

  it('renders "Launch Game" + "Launch Trainer" buttons in in-place mode', () => {
    renderCommandSection({
      onLaunchGame: vi.fn(),
      onLaunchTrainer: vi.fn(),
      canLaunchGame: true,
      canLaunchTrainer: true,
    });
    expect(screen.getByRole('button', { name: 'Launch Game' })).not.toBeDisabled();
    expect(screen.getByRole('button', { name: 'Launch Trainer' })).not.toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('Launch Game and Launch Trainer are disabled when canLaunch flags are false', () => {
    renderCommandSection({
      onLaunchGame: vi.fn(),
      onLaunchTrainer: vi.fn(),
      canLaunchGame: false,
      canLaunchTrainer: false,
    });
    expect(screen.getByRole('button', { name: 'Launch Game' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Launch Trainer' })).toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('in-place Launch Game calls onBeforeLaunch then onLaunchGame when proceed=true', async () => {
    const user = userEvent.setup();
    const onBeforeLaunch = vi.fn().mockResolvedValue(true);
    const onLaunchGame = vi.fn();
    renderCommandSection({
      onLaunchGame,
      onLaunchTrainer: vi.fn(),
      canLaunchGame: true,
      onBeforeLaunch,
    });

    await user.click(screen.getByRole('button', { name: 'Launch Game' }));
    expect(onBeforeLaunch).toHaveBeenCalledWith('game');
    await waitFor(() => expect(onLaunchGame).toHaveBeenCalled());
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('in-place Launch Game does NOT call onLaunchGame when onBeforeLaunch returns false', async () => {
    const user = userEvent.setup();
    const onBeforeLaunch = vi.fn().mockResolvedValue(false);
    const onLaunchGame = vi.fn();
    renderCommandSection({
      onLaunchGame,
      onLaunchTrainer: vi.fn(),
      canLaunchGame: true,
      onBeforeLaunch,
    });

    await user.click(screen.getByRole('button', { name: 'Launch Game' }));
    expect(onBeforeLaunch).toHaveBeenCalledWith('game');
    // Give any async effects time to settle.
    await act(async () => {});
    expect(onLaunchGame).not.toHaveBeenCalled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('in-place Launch Trainer calls onBeforeLaunch("trainer") then onLaunchTrainer', async () => {
    const user = userEvent.setup();
    const onBeforeLaunch = vi.fn().mockResolvedValue(true);
    const onLaunchTrainer = vi.fn();
    renderCommandSection({
      onLaunchGame: vi.fn(),
      onLaunchTrainer,
      canLaunchTrainer: true,
      onBeforeLaunch,
    });

    await user.click(screen.getByRole('button', { name: 'Launch Trainer' }));
    expect(onBeforeLaunch).toHaveBeenCalledWith('trainer');
    await waitFor(() => expect(onLaunchTrainer).toHaveBeenCalled());
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('shows "Game Running" label when isGameRunning=true', () => {
    renderCommandSection({
      onLaunchGame: vi.fn(),
      onLaunchTrainer: vi.fn(),
      isGameRunning: true,
      canLaunchGame: false,
      phase: LaunchPhase.Idle,
    });
    expect(screen.getByRole('button', { name: 'Game Running' })).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
