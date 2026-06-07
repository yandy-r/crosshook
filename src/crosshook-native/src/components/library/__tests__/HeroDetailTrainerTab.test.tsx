import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { launchOptimizationsAutosaveDelayMs } from '@/hooks/profile/constants';
import { emitMockEvent, resetBrowserEventBus } from '@/lib/events';
import { makeLibraryCardData, makeProfileDraft } from '@/test/fixtures';
import type { InjectionLogEvent } from '@/types/injection';
import type { GameProfile, LoadedDllHook } from '@/types/profile';
import { HeroDetailTrainerTab } from '../HeroDetailTrainerTab';

const profileContextMock = vi.fn();
const updateProfileSpy = vi.fn();
const persistProfileDraftSpy = vi.fn();

vi.mock('@/context/ProfileContext', () => ({
  useProfileContext: () => profileContextMock(),
}));

function makeHook(overrides: Partial<LoadedDllHook> = {}): LoadedDllHook {
  return {
    id: 'dll-1',
    name: 'Overlay bootstrap',
    path: '/hooks/overlay.dll',
    enabled: true,
    ...overrides,
  };
}

function makeProfile(overrides: Partial<GameProfile> = {}): GameProfile {
  return makeProfileDraft({
    game: { name: 'Synthetic Quest', executable_path: '/games/synthetic-quest/game.exe' },
    trainer: { path: '/trainers/synthetic-quest/trainer.exe', type: '', loading_mode: 'source_directory' },
    injection: {
      loaded_hooks: [makeHook()],
      method: 'disabled',
      stage: 'trainer_launch',
      timeout_ms: 0,
      fallback: 'warn_and_continue',
    },
    ...overrides,
  });
}

function makeInjectionEvent(overrides: Partial<InjectionLogEvent> = {}): InjectionLogEvent {
  return {
    timestamp: '2026-06-05T12:00:00.000Z',
    profile_name: 'Synthetic Quest',
    session_id: 'session-1',
    session_kind: 'trainer',
    level: 'info',
    source: 'trainer',
    message: 'Trainer event recorded.',
    ...overrides,
  };
}

function renderTrainerTab(
  options: {
    profile?: GameProfile;
    profileName?: string;
    selectedProfile?: string;
    profiles?: string[];
    displayProfileName?: string;
  } = {}
) {
  let profile = options.profile ?? makeProfile();
  const profileName = options.profileName ?? 'Synthetic Quest';
  const selectedProfile = options.selectedProfile ?? 'Synthetic Quest';
  const profiles = options.profiles ?? ['Synthetic Quest'];

  function refreshContext() {
    profileContextMock.mockReturnValue({
      profile,
      profileName,
      selectedProfile,
      profiles,
      updateProfile: updateProfileSpy,
      persistProfileDraft: persistProfileDraftSpy,
    });
  }

  updateProfileSpy.mockImplementation((updater: GameProfile | ((current: GameProfile) => GameProfile)) => {
    profile = typeof updater === 'function' ? updater(profile) : updater;
    refreshContext();
    view.rerender(
      <HeroDetailTrainerTab
        summary={makeLibraryCardData()}
        displayProfileName={options.displayProfileName ?? 'Synthetic Quest'}
      />
    );
  });

  refreshContext();
  const view = render(
    <HeroDetailTrainerTab
      summary={makeLibraryCardData()}
      displayProfileName={options.displayProfileName ?? 'Synthetic Quest'}
    />
  );

  return {
    ...view,
    get profile() {
      return profile;
    },
  };
}

async function flushTrainerAutosave() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(launchOptimizationsAutosaveDelayMs);
    await Promise.resolve();
  });
}

describe('HeroDetailTrainerTab', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.clearAllMocks();
    resetBrowserEventBus();
    persistProfileDraftSpy.mockResolvedValue({ ok: true });
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    resetBrowserEventBus();
    consoleErrorSpy.mockRestore();
  });

  it('renders loaded hooks, stored-only configuration, and recent log sections', () => {
    renderTrainerTab();

    expect(screen.getAllByRole('heading', { name: 'Loaded DLL hooks' })).toHaveLength(2);
    expect(screen.getByRole('heading', { name: 'Injection configuration' })).toBeInTheDocument();
    expect(screen.getAllByRole('heading', { name: 'Recent injection log' })).toHaveLength(2);
    expect(screen.getByText('Overlay bootstrap')).toBeInTheDocument();
    expect(screen.getByText('/hooks/overlay.dll')).toBeInTheDocument();
    expect(screen.getByText('Stored only')).toBeInTheDocument();
    expect(
      screen.getByText(
        'DLL injection settings are stored on the profile. No DLL injection engine runs from this editor yet.'
      )
    ).toBeInTheDocument();
    expect(screen.getByRole('status', { name: '' })).toHaveTextContent(
      'No trainer or injection events recorded for this profile in this session.'
    );
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('adds, edits, toggles, and removes DLL hooks while mirroring legacy injection fields', async () => {
    vi.useFakeTimers();
    try {
      const view = renderTrainerTab({
        profile: makeProfile({ injection: { loaded_hooks: [], timeout_ms: 0 } }),
      });

      fireEvent.click(screen.getByRole('button', { name: '+ Attach DLL' }));
      expect(view.profile.injection.loaded_hooks).toEqual([
        expect.objectContaining({ name: 'Loaded DLL hook', path: '', enabled: true }),
      ]);
      expect(view.profile.injection.dll_paths).toEqual(['']);
      expect(view.profile.injection.inject_on_launch).toEqual([true]);

      fireEvent.click(screen.getByRole('button', { name: 'Edit Loaded DLL hook' }));
      let popover = screen.getByText('DLL path').closest('.crosshook-hero-detail__hook-popover');
      expect(popover).not.toBeNull();

      fireEvent.change(within(popover as HTMLElement).getByLabelText('Name'), {
        target: { value: 'HUD bridge' },
      });
      popover = screen.getByText('DLL path').closest('.crosshook-hero-detail__hook-popover');
      fireEvent.change(within(popover as HTMLElement).getByLabelText('DLL path'), {
        target: { value: '/hooks/hud-bridge.dll' },
      });
      fireEvent.click(screen.getByRole('checkbox', { name: 'Enabled' }));

      expect(view.profile.injection.loaded_hooks[0]).toEqual(
        expect.objectContaining({
          name: 'HUD bridge',
          path: '/hooks/hud-bridge.dll',
          enabled: false,
        })
      );
      expect(view.profile.injection.dll_paths).toEqual(['/hooks/hud-bridge.dll']);
      expect(view.profile.injection.inject_on_launch).toEqual([false]);

      popover = screen.getByText('DLL path').closest('.crosshook-hero-detail__hook-popover');
      fireEvent.click(within(popover as HTMLElement).getByRole('button', { name: 'Remove' }));

      expect(view.profile.injection.loaded_hooks).toEqual([]);
      expect(view.profile.injection.dll_paths).toEqual([]);
      expect(view.profile.injection.inject_on_launch).toEqual([]);

      await flushTrainerAutosave();
      expect(persistProfileDraftSpy).toHaveBeenCalledWith(
        'Synthetic Quest',
        expect.objectContaining({
          injection: expect.objectContaining({
            loaded_hooks: [],
            dll_paths: [],
            inject_on_launch: [],
          }),
        })
      );
      expect(consoleErrorSpy).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('edits injection config and reports debounced save success', async () => {
    vi.useFakeTimers();
    try {
      const view = renderTrainerTab();

      fireEvent.change(screen.getByLabelText('Timeout (ms)'), { target: { value: '2500' } });

      expect(view.profile.injection.timeout_ms).toBe(2500);
      await flushTrainerAutosave();
      expect(screen.getByText('Trainer settings saved')).toBeInTheDocument();
      expect(persistProfileDraftSpy).toHaveBeenCalledWith(
        'Synthetic Quest',
        expect.objectContaining({
          injection: expect.objectContaining({ timeout_ms: 2500 }),
        })
      );
      expect(consoleErrorSpy).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('reports debounced save errors without dropping the edited draft', async () => {
    vi.useFakeTimers();
    try {
      persistProfileDraftSpy.mockResolvedValue({ ok: false, error: 'Disk is full.' });
      const view = renderTrainerTab();

      fireEvent.change(screen.getByLabelText('Timeout (ms)'), { target: { value: '1200' } });
      expect(view.profile.injection.timeout_ms).toBe(1200);

      await flushTrainerAutosave();
      expect(screen.getByText('Trainer settings failed to save')).toBeInTheDocument();
      expect(screen.getByText('Disk is full.')).toHaveAttribute('role', 'status');
      expect(persistProfileDraftSpy).toHaveBeenCalledWith(
        'Synthetic Quest',
        expect.objectContaining({
          injection: expect.objectContaining({ timeout_ms: 1200 }),
        })
      );
      expect(consoleErrorSpy).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('does not mount write controls or save when the displayed profile mismatches the selected profile', async () => {
    vi.useFakeTimers();
    try {
      renderTrainerTab({ displayProfileName: 'Other Quest' });

      expect(
        screen.getByText(/Trainer settings apply to the selected profile \(Synthetic Quest\)/)
      ).toBeInTheDocument();
      expect(screen.getByText('Select Other Quest to edit stored injection configuration.')).toBeInTheDocument();
      expect(screen.queryByRole('button', { name: '+ Attach DLL' })).not.toBeInTheDocument();
      expect(screen.queryByLabelText('Timeout (ms)')).not.toBeInTheDocument();

      await flushTrainerAutosave();

      expect(updateProfileSpy).not.toHaveBeenCalled();
      expect(persistProfileDraftSpy).not.toHaveBeenCalled();
      expect(consoleErrorSpy).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('renders stored-only unsupported runtime messaging for matching injection log events', async () => {
    renderTrainerTab();

    await act(async () => {});
    act(() => {
      emitMockEvent(
        'injection-log',
        makeInjectionEvent({
          level: 'warning',
          source: 'injection',
          message: 'DLL injection engine is not available.',
          hook_name: 'HUD bridge',
          unsupported_runtime: true,
        })
      );
    });

    expect(await screen.findByText('DLL injection engine is not available.')).toBeInTheDocument();
    expect(screen.getByText('HUD bridge')).toBeInTheDocument();
    expect(screen.getByText('Stored configuration only; no DLL injection engine ran.')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('filters injection log events by displayed profile name', async () => {
    renderTrainerTab();

    await act(async () => {});
    act(() => {
      emitMockEvent('injection-log', makeInjectionEvent({ profile_name: 'Other Quest', message: 'Ignored event.' }));
      emitMockEvent('injection-log', makeInjectionEvent({ message: 'Visible trainer event.' }));
      emitMockEvent('injection-log', { message: 'Malformed event.' });
    });

    expect(await screen.findByText('Visible trainer event.')).toBeInTheDocument();
    expect(screen.queryByText('Ignored event.')).not.toBeInTheDocument();
    expect(screen.queryByText('Malformed event.')).not.toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });

  it('caps the recent injection log at 200 rows', async () => {
    renderTrainerTab();

    await act(async () => {});
    act(() => {
      for (let index = 0; index < 205; index += 1) {
        emitMockEvent(
          'injection-log',
          makeInjectionEvent({
            timestamp: `2026-06-05T12:${String(Math.floor(index / 60)).padStart(2, '0')}:${String(index % 60).padStart(
              2,
              '0'
            )}.000Z`,
            session_id: `session-${index}`,
            message: `Trainer log row ${index}`,
          })
        );
      }
    });

    expect(await screen.findByText('Trainer log row 204')).toBeInTheDocument();
    expect(screen.queryByText('Trainer log row 0')).not.toBeInTheDocument();
    expect(screen.queryByText('Trainer log row 4')).not.toBeInTheDocument();
    expect(screen.getByText('Trainer log row 5')).toBeInTheDocument();
    expect(screen.getByText('200')).toBeInTheDocument();
    expect(consoleErrorSpy).not.toHaveBeenCalled();
  });
});
