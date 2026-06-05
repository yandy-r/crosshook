/**
 * Focused tests for HeroLaunchGate.
 *
 * Tests:
 *  - selectProfile-first gate: selectProfile called before launch when
 *    displayed ≠ selected profile; abort + hint when profile not selectable.
 *  - Dep-gate modal flow: gated launch → modal renders, confirm / cancel paths.
 *  - LaunchPanelFeedback rendered when feedback is present.
 *  - LaunchPipeline presence in DOM.
 *  - notSelectableHint: controls disabled + hint text still rendered (disabled-not-removed).
 *  - profileMismatch forwarded to HeroLaunchSubTabsHost.
 *
 * Strategy A: hand-rolled vi.mock of all contexts, hooks, and sub-components.
 * HeroLaunchCommandSection and HeroLaunchSubTabsHost are stubbed so tests
 * stay focused on HeroLaunchGate's own orchestration logic.
 */
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { makeLaunchPreview, makeLaunchRequest, makeProfileDraft } from '@/test/fixtures';
import { LaunchPhase } from '@/types/launch';
import { HeroLaunchGate } from '../launch/HeroLaunchGate';

// ── Context mocks ─────────────────────────────────────────────────────────────

const profileContextMock = vi.fn();
const preferencesContextMock = vi.fn();
const launchStateContextMock = vi.fn();

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

vi.mock('@/context/PreferencesContext', () => ({
  usePreferencesContext: () => preferencesContextMock(),
}));

vi.mock('@/context/LaunchStateContext', () => ({
  useLaunchStateContext: () => launchStateContextMock(),
}));

// ── Hook mocks ────────────────────────────────────────────────────────────────

const useLaunchDepGateMock = vi.fn();
vi.mock('@/components/library/launch/useLaunchDepGate', () => ({
  useLaunchDepGate: (opts: unknown) => useLaunchDepGateMock(opts),
}));

const copyToClipboardMock = vi.fn();
vi.mock('@/utils/clipboard', () => ({
  copyToClipboard: (text: string) => copyToClipboardMock(text),
}));

// ── Sub-component mocks ───────────────────────────────────────────────────────

// HeroLaunchCommandSection: capture onBeforeLaunch so we can trigger it from tests.
let capturedOnBeforeLaunch: ((action: 'game' | 'trainer') => Promise<boolean>) | undefined;
let _capturedOnLaunchGame: (() => void) | undefined;
let _capturedOnLaunchTrainer: (() => void) | undefined;

vi.mock('@/components/library/launch/HeroLaunchCommandSection', () => ({
  HeroLaunchCommandSection: (props: {
    onBeforeLaunch?: (action: 'game' | 'trainer') => Promise<boolean>;
    onLaunchGame?: () => void;
    onLaunchTrainer?: () => void;
    notSelectableHint?: string | null;
    canLaunchGame?: boolean;
    canLaunchTrainer?: boolean;
    launchRequest: unknown;
    preview: unknown;
    previewLoading: boolean;
    previewError: string | null;
    resolvedProfileName: string;
    isLaunching: boolean;
  }) => {
    capturedOnBeforeLaunch = props.onBeforeLaunch;
    _capturedOnLaunchGame = props.onLaunchGame;
    _capturedOnLaunchTrainer = props.onLaunchTrainer;
    return (
      <div data-testid="command-section">
        <button
          type="button"
          disabled={!props.canLaunchGame}
          onClick={() => void props.onBeforeLaunch?.('game').then((ok) => ok && props.onLaunchGame?.())}
        >
          Launch Game
        </button>
        <button
          type="button"
          disabled={!props.canLaunchTrainer}
          onClick={() => void props.onBeforeLaunch?.('trainer').then((ok) => ok && props.onLaunchTrainer?.())}
        >
          Launch Trainer
        </button>
        {props.notSelectableHint ? <p role="note">{props.notSelectableHint}</p> : null}
      </div>
    );
  },
}));

vi.mock('@/components/library/launch/HeroLaunchSubTabsHost', () => ({
  HeroLaunchSubTabsHost: (props: { profileMismatch: boolean; isGamescopeRunning?: boolean }) => (
    <div
      data-testid="subtabs-host"
      data-mismatch={String(props.profileMismatch)}
      data-gamescope={String(props.isGamescopeRunning ?? false)}
    />
  ),
}));

vi.mock('@/components/LaunchPipeline', () => ({
  LaunchPipeline: () => <div data-testid="launch-pipeline" />,
}));

vi.mock('@/components/launch-panel/LaunchPanelFeedback', () => ({
  LaunchPanelFeedback: (props: {
    feedback: { kind: string };
    diagnosticCopyLabel: string;
    onCopyDiagnosticReport: () => void;
  }) => (
    <div role="alert" data-testid="launch-feedback" data-kind={props.feedback.kind}>
      <button type="button" onClick={props.onCopyDiagnosticReport}>
        {props.diagnosticCopyLabel}
      </button>
    </div>
  ),
}));

vi.mock('@/components/library/launch/LaunchDepGateModal', () => ({
  LaunchDepGateModal: (props: { depGate: { depGatePackages: string[] | null } }) =>
    props.depGate.depGatePackages ? (
      <div role="dialog" aria-label="dep-gate-modal">
        Missing dependencies
      </div>
    ) : null,
}));

// ── Helpers ───────────────────────────────────────────────────────────────────

const selectProfileSpy = vi.fn();
const launchGameSpy = vi.fn();
const launchTrainerSpy = vi.fn();

function makeBaseDepGate(overrides: Record<string, unknown> = {}) {
  return {
    depGatePackages: null,
    depGatePendingAction: null,
    depGateInstalling: false,
    isGamescopeRunning: false,
    setDepGatePackages: vi.fn(),
    setDepGatePendingAction: vi.fn(),
    setDepGateInstalling: vi.fn(),
    handleBeforeLaunch: vi.fn().mockResolvedValue(true),
    installPrefixDependency: vi.fn(),
    ...overrides,
  };
}

function makeProfileCtx(overrides: Record<string, unknown> = {}) {
  return {
    profile: makeProfileDraft({
      game: { name: 'Synthetic Quest', executable_path: '/games/synthetic-quest/game.exe' },
      trainer: { path: '/trainers/synthetic-quest/trainer.exe', type: 'dll', loading_mode: 'source_directory' },
      runtime: { prefix_path: '/prefixes/synthetic-quest', proton_path: '/compat/proton', working_directory: '' },
      launch: {
        method: 'proton_run',
        optimizations: { enabled_option_ids: [] },
        custom_env_vars: {},
      },
    }),
    profileName: 'Synthetic Quest',
    selectedProfile: 'Synthetic Quest',
    profiles: ['Synthetic Quest'],
    selectProfile: selectProfileSpy,
    activeCollectionId: null,
    ...overrides,
  };
}

function makeLaunchStateCtx(overrides: Record<string, unknown> = {}) {
  return {
    canLaunchGame: true,
    canLaunchTrainer: true,
    feedback: null,
    helperLogPath: null,
    hintText: '',
    isBusy: false,
    isGameRunning: false,
    launchGame: launchGameSpy,
    launchTrainer: launchTrainerSpy,
    phase: LaunchPhase.Idle,
    statusText: '',
    ...overrides,
  };
}

function renderGate(props: Partial<React.ComponentProps<typeof HeroLaunchGate>> = {}) {
  const depGate = makeBaseDepGate();
  profileContextMock.mockReturnValue(makeProfileCtx());
  preferencesContextMock.mockReturnValue({
    settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
  });
  launchStateContextMock.mockReturnValue(makeLaunchStateCtx());
  useLaunchDepGateMock.mockReturnValue(depGate);

  return render(
    <HeroLaunchGate
      launchRequest={makeLaunchRequest()}
      previewLoading={false}
      preview={makeLaunchPreview()}
      previewError={null}
      resolvedProfileName="Synthetic Quest"
      resolvedSteamAppId="9999001"
      hasSavedSelectedProfile={true}
      profileMismatch={false}
      displayProfileName="Synthetic Quest"
      isLaunching={false}
      {...props}
    />
  );
}

describe('HeroLaunchGate', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    capturedOnBeforeLaunch = undefined;
    _capturedOnLaunchGame = undefined;
    _capturedOnLaunchTrainer = undefined;
    selectProfileSpy.mockResolvedValue(undefined);
    launchGameSpy.mockResolvedValue(undefined);
    launchTrainerSpy.mockResolvedValue(undefined);
    copyToClipboardMock.mockResolvedValue(undefined);
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    consoleErrorSpy.mockRestore();
    vi.useRealTimers();
  });

  // ── Basic rendering ──────────────────────────────────────────────────────────

  it('renders the command section, subtabs host, and pipeline', () => {
    renderGate();
    expect(screen.getByTestId('command-section')).toBeInTheDocument();
    expect(screen.getByTestId('subtabs-host')).toBeInTheDocument();
    expect(screen.getByTestId('launch-pipeline')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('does not render feedback when feedback is null', () => {
    renderGate();
    expect(screen.queryByTestId('launch-feedback')).not.toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders LaunchPanelFeedback when feedback is set', () => {
    const depGate = makeBaseDepGate();
    profileContextMock.mockReturnValue(makeProfileCtx());
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(
      makeLaunchStateCtx({
        feedback: { kind: 'runtime', message: 'Process exited with code 1' },
      })
    );
    useLaunchDepGateMock.mockReturnValue(depGate);

    render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={true}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );
    expect(screen.getByTestId('launch-feedback')).toBeInTheDocument();
    expect(screen.getByTestId('launch-feedback')).toHaveAttribute('data-kind', 'runtime');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('clears stale diagnostic copy reset timers across repeated clicks and unmount', async () => {
    vi.useFakeTimers();
    const clearTimeoutSpy = vi.spyOn(window, 'clearTimeout');
    const depGate = makeBaseDepGate();
    profileContextMock.mockReturnValue(makeProfileCtx());
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(
      makeLaunchStateCtx({
        feedback: {
          kind: 'diagnostic',
          report: {
            summary: 'Launch failed',
            exit_info: { description: 'Synthetic diagnostic', failure_mode: 'process_exit', code: 1, signal: null },
            pattern_matches: [],
            suggestions: [],
            severity: 'warning',
            analyzed_at: '2026-06-04T16:07:10-04:00',
            log_tail_path: null,
          },
        },
      })
    );
    useLaunchDepGateMock.mockReturnValue(depGate);

    const { unmount } = render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={true}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Copy Report' }));
    });
    expect(screen.getByRole('button', { name: 'Copied!' })).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Copied!' }));
    });

    expect(clearTimeoutSpy).toHaveBeenCalledTimes(1);
    unmount();
    expect(clearTimeoutSpy).toHaveBeenCalledTimes(2);

    await act(async () => {
      vi.runOnlyPendingTimers();
    });

    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('forwards isGamescopeRunning from depGate to HeroLaunchSubTabsHost', () => {
    const depGate = makeBaseDepGate({ isGamescopeRunning: true });
    profileContextMock.mockReturnValue(makeProfileCtx());
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(makeLaunchStateCtx());
    useLaunchDepGateMock.mockReturnValue(depGate);

    render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={true}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );

    expect(screen.getByTestId('subtabs-host')).toHaveAttribute('data-gamescope', 'true');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── selectProfile-first gate ─────────────────────────────────────────────────

  it('selects the displayed profile and aborts the current launch when displayed ≠ selected profile', async () => {
    const depGate = makeBaseDepGate({ handleBeforeLaunch: vi.fn().mockResolvedValue(true) });
    profileContextMock.mockReturnValue(
      makeProfileCtx({
        selectedProfile: 'Other Profile',
        profiles: ['Synthetic Quest', 'Other Profile'],
      })
    );
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(makeLaunchStateCtx({ canLaunchGame: true }));
    useLaunchDepGateMock.mockReturnValue(depGate);

    render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={true}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );

    expect(capturedOnBeforeLaunch).toBeDefined();
    let result: boolean | undefined;
    await act(async () => {
      result = await capturedOnBeforeLaunch?.('game');
    });

    expect(selectProfileSpy).toHaveBeenCalledWith(
      'Synthetic Quest',
      expect.objectContaining({ collectionId: undefined })
    );
    expect(depGate.handleBeforeLaunch).not.toHaveBeenCalled();
    expect(result).toBe(false);
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('does NOT call selectProfile when displayed === selected', async () => {
    const depGate = makeBaseDepGate({ handleBeforeLaunch: vi.fn().mockResolvedValue(true) });
    profileContextMock.mockReturnValue(makeProfileCtx({ selectedProfile: 'Synthetic Quest' }));
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(makeLaunchStateCtx());
    useLaunchDepGateMock.mockReturnValue(depGate);

    renderGate({ displayProfileName: 'Synthetic Quest' });

    await act(async () => {
      await capturedOnBeforeLaunch?.('game');
    });

    expect(selectProfileSpy).not.toHaveBeenCalled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('aborts launch (returns false) when displayed profile is not in profiles list', async () => {
    const depGate = makeBaseDepGate({ handleBeforeLaunch: vi.fn().mockResolvedValue(true) });
    profileContextMock.mockReturnValue(
      makeProfileCtx({
        selectedProfile: 'Other Profile',
        profiles: ['Other Profile'],
        // 'Synthetic Quest' is NOT in the list
      })
    );
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(makeLaunchStateCtx());
    useLaunchDepGateMock.mockReturnValue(depGate);

    render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={true}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );

    let result: boolean | undefined;
    await act(async () => {
      result = await capturedOnBeforeLaunch?.('game');
    });

    expect(result).toBe(false);
    expect(selectProfileSpy).not.toHaveBeenCalled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── notSelectableHint: disabled-not-removed ──────────────────────────────────

  it('passes notSelectableHint when profile is not in profiles list', () => {
    profileContextMock.mockReturnValue(
      makeProfileCtx({
        selectedProfile: 'Other Profile',
        profiles: ['Other Profile'],
      })
    );
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(makeLaunchStateCtx({ canLaunchGame: false }));
    useLaunchDepGateMock.mockReturnValue(makeBaseDepGate());

    render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={false}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );

    const note = screen.getByRole('note');
    expect(note).toHaveTextContent(/not saved and cannot be launched/i);
    // Command section is still in DOM (disabled-not-removed).
    expect(screen.getByTestId('command-section')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── Dep-gate modal ────────────────────────────────────────────────────────────

  it('does not render the dep-gate modal when depGatePackages is null', () => {
    renderGate();
    expect(screen.queryByRole('dialog', { name: 'dep-gate-modal' })).not.toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('renders the dep-gate modal when depGatePackages is not null', () => {
    const depGate = makeBaseDepGate({ depGatePackages: ['vcrun2019'] });
    profileContextMock.mockReturnValue(makeProfileCtx());
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(makeLaunchStateCtx());
    useLaunchDepGateMock.mockReturnValue(depGate);

    render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={true}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );

    expect(screen.getByRole('dialog', { name: 'dep-gate-modal' })).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── In-place launch orchestration ─────────────────────────────────────────────

  it('canLaunchGame is gated by profileSelectable (false when not in list)', () => {
    profileContextMock.mockReturnValue(
      makeProfileCtx({
        selectedProfile: 'Other Profile',
        profiles: ['Other Profile'],
      })
    );
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(makeLaunchStateCtx({ canLaunchGame: true }));
    useLaunchDepGateMock.mockReturnValue(makeBaseDepGate());

    render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={false}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );

    // The Launch Game button (from our stub) is disabled because canLaunchGame
    // is false (profileSelectable is false).
    expect(screen.getByRole('button', { name: 'Launch Game' })).toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('Launch Game button is enabled when profile is selectable and canLaunchGame is true', () => {
    renderGate();
    expect(screen.getByRole('button', { name: 'Launch Game' })).not.toBeDisabled();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('calls launchGame after selectProfile and depGate pass', async () => {
    const user = userEvent.setup();
    const handleBeforeLaunch = vi.fn().mockResolvedValue(true);
    const depGate = makeBaseDepGate({ handleBeforeLaunch });
    profileContextMock.mockReturnValue(makeProfileCtx());
    preferencesContextMock.mockReturnValue({
      settings: { umu_preference: 'auto', auto_install_prefix_deps: false },
    });
    launchStateContextMock.mockReturnValue(makeLaunchStateCtx({ canLaunchGame: true }));
    useLaunchDepGateMock.mockReturnValue(depGate);

    render(
      <HeroLaunchGate
        launchRequest={makeLaunchRequest()}
        previewLoading={false}
        preview={makeLaunchPreview()}
        previewError={null}
        resolvedProfileName="Synthetic Quest"
        resolvedSteamAppId="9999001"
        hasSavedSelectedProfile={true}
        profileMismatch={false}
        displayProfileName="Synthetic Quest"
        isLaunching={false}
      />
    );

    await user.click(screen.getByRole('button', { name: 'Launch Game' }));
    await waitFor(() => expect(launchGameSpy).toHaveBeenCalled());
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  // ── profileMismatch forwarded ────────────────────────────────────────────────

  it('forwards profileMismatch=true to HeroLaunchSubTabsHost', () => {
    renderGate({ profileMismatch: true });
    expect(screen.getByTestId('subtabs-host')).toHaveAttribute('data-mismatch', 'true');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('forwards profileMismatch=false to HeroLaunchSubTabsHost', () => {
    renderGate({ profileMismatch: false });
    expect(screen.getByTestId('subtabs-host')).toHaveAttribute('data-mismatch', 'false');
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
