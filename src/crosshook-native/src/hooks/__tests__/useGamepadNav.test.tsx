import { act, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
// `subscribeGamepadYAction` is imported through the public module on purpose: the re-export is part of the hook's surface.
import { type GamepadNavState, subscribeGamepadYAction, useGamepadNav } from '../useGamepadNav';

interface HarnessProps {
  enabled?: boolean;
  onBack?: () => void;
}

let latestState: GamepadNavState | null = null;

function GamepadNavHarness({ enabled = false, onBack }: HarnessProps) {
  const nav = useGamepadNav({ enabled, onBack });
  latestState = nav;

  return (
    <div
      ref={(node) => {
        nav.rootRef.current = node;
      }}
      data-testid="nav-root"
    >
      <nav data-crosshook-focus-zone="sidebar">
        <button type="button" value="library" data-state="active">
          Library
        </button>
        <button type="button" value="profiles">
          Profiles
        </button>
      </nav>
      <main data-crosshook-focus-zone="content">
        <button type="button">Launch</button>
        <button type="button">Edit</button>
      </main>
    </div>
  );
}

function makeGamepad(buttons: number[] = [], axes: number[] = [0, 0]): Gamepad {
  const pressed = new Set(buttons);
  return {
    axes,
    buttons: Array.from({ length: 16 }, (_, index) => ({
      pressed: pressed.has(index),
      touched: pressed.has(index),
      value: pressed.has(index) ? 1 : 0,
    })),
    connected: true,
    id: 'Mock Gamepad',
    index: 0,
    mapping: 'standard',
    timestamp: Date.now(),
    vibrationActuator: null,
    hapticActuators: [],
  } as unknown as Gamepad;
}

describe('useGamepadNav', () => {
  beforeEach(() => {
    latestState = null;
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('cycles focus with capture-phase keyboard handling', () => {
    render(<GamepadNavHarness enabled={false} />);

    const launchButton = screen.getByRole('button', { name: 'Launch' });
    const editButton = screen.getByRole('button', { name: 'Edit' });

    launchButton.focus();
    expect(launchButton).toHaveFocus();

    act(() => {
      launchButton.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true, cancelable: true }));
    });
    expect(editButton).toHaveFocus();
  });

  it('moves focus from gamepad polling when controller mode is enabled', () => {
    vi.useFakeTimers();
    vi.spyOn(navigator, 'getGamepads').mockReturnValue([makeGamepad([13])] as unknown as Gamepad[]);

    render(<GamepadNavHarness enabled />);

    const launchButton = screen.getByRole('button', { name: 'Launch' });
    const editButton = screen.getByRole('button', { name: 'Edit' });
    launchButton.focus();

    act(() => {
      vi.advanceTimersByTime(20);
    });

    expect(editButton).toHaveFocus();
  });

  it('schedules no animation frames while controller mode is off, even with a gamepad connected', () => {
    vi.useFakeTimers();
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');
    // Y is held down the whole time: with no polling loop there is nothing to detect it.
    vi.spyOn(navigator, 'getGamepads').mockReturnValue([makeGamepad([3])] as unknown as Gamepad[]);
    const onY = vi.fn();
    const unsubscribe = subscribeGamepadYAction(onY);

    render(<GamepadNavHarness enabled={false} />);
    act(() => {
      vi.advanceTimersByTime(500);
    });

    expect(rafSpy).not.toHaveBeenCalled();
    expect(onY).not.toHaveBeenCalled();
    unsubscribe();
  });

  it('stops the polling loop when controller mode is switched off', () => {
    vi.useFakeTimers();
    // Count executed polls (each frame reads the gamepad once), not `requestAnimationFrame` calls:
    // re-running the effect cancels and re-schedules a frame without polling an extra time.
    const getGamepadsSpy = vi
      .spyOn(navigator, 'getGamepads')
      .mockReturnValue([makeGamepad([])] as unknown as Gamepad[]);

    const { rerender } = render(<GamepadNavHarness enabled />);
    const advanceFrame = () =>
      act(() => {
        vi.advanceTimersByTime(20);
      });

    advanceFrame();
    advanceFrame();
    const pollsWhileOn = getGamepadsSpy.mock.calls.length;
    expect(pollsWhileOn).toBeGreaterThan(1);

    rerender(<GamepadNavHarness enabled={false} />);
    advanceFrame();
    advanceFrame();
    advanceFrame();

    // Not one further frame reads the gamepad once controller mode is off.
    expect(getGamepadsSpy.mock.calls.length).toBe(pollsWhileOn);
  });

  it('notifies Y-action listeners on the rising edge of button 3 only', () => {
    vi.useFakeTimers();
    let yPressed = false;
    vi.spyOn(navigator, 'getGamepads').mockImplementation(
      () => [makeGamepad(yPressed ? [3] : [])] as unknown as Gamepad[]
    );
    const onY = vi.fn();
    const unsubscribe = subscribeGamepadYAction(onY);

    render(<GamepadNavHarness enabled />);
    const advanceFrame = () =>
      act(() => {
        vi.advanceTimersByTime(20);
      });

    advanceFrame();
    expect(onY).not.toHaveBeenCalled();

    yPressed = true;
    advanceFrame();
    expect(onY).toHaveBeenCalledTimes(1);

    // Still held: no new edge.
    advanceFrame();
    advanceFrame();
    expect(onY).toHaveBeenCalledTimes(1);

    yPressed = false;
    advanceFrame();
    expect(onY).toHaveBeenCalledTimes(1);

    yPressed = true;
    advanceFrame();
    expect(onY).toHaveBeenCalledTimes(2);

    unsubscribe();
    yPressed = false;
    advanceFrame();
    yPressed = true;
    advanceFrame();
    expect(onY).toHaveBeenCalledTimes(2);
  });

  it('isolates a throwing Y-action listener so the poll loop and other listeners keep running', () => {
    vi.useFakeTimers();
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    let yPressed = true;
    vi.spyOn(navigator, 'getGamepads').mockImplementation(
      () => [makeGamepad(yPressed ? [3] : [])] as unknown as Gamepad[]
    );

    const healthy = vi.fn();
    const unsubThrow = subscribeGamepadYAction(() => {
      throw new Error('boom');
    });
    const unsubHealthy = subscribeGamepadYAction(healthy);

    render(<GamepadNavHarness enabled />);

    const advanceFrame = () => {
      act(() => {
        vi.advanceTimersByTime(16);
      });
    };

    try {
      advanceFrame();
      expect(healthy).toHaveBeenCalledTimes(1);
      expect(errorSpy).toHaveBeenCalledTimes(1);

      // Poll loop survived: releasing and pressing Y fires healthy again.
      yPressed = false;
      advanceFrame();
      yPressed = true;
      advanceFrame();
      expect(healthy).toHaveBeenCalledTimes(2);
    } finally {
      unsubThrow();
      unsubHealthy();
    }
  });

  it('delivers the Y action even when a navigation button is pressed in the same frame', () => {
    vi.useFakeTimers();
    vi.spyOn(navigator, 'getGamepads').mockReturnValue([makeGamepad([13, 3])] as unknown as Gamepad[]);

    const onY = vi.fn();
    const unsubscribe = subscribeGamepadYAction(onY);

    try {
      render(<GamepadNavHarness enabled />);
      const launchButton = screen.getByRole('button', { name: 'Launch' });
      const editButton = screen.getByRole('button', { name: 'Edit' });
      launchButton.focus();

      act(() => {
        vi.advanceTimersByTime(16);
      });

      expect(editButton).toHaveFocus();
      expect(onY).toHaveBeenCalledTimes(1);
    } finally {
      unsubscribe();
    }
  });

  it('moves focus to content zone when back() is called from inspector zone', async () => {
    function InspectorHarness({ onBack }: HarnessProps) {
      const nav = useGamepadNav({ enabled: true, onBack });
      latestState = nav;

      return (
        <div
          ref={(node) => {
            nav.rootRef.current = node;
          }}
          data-testid="nav-root"
        >
          <nav data-crosshook-focus-zone="sidebar">
            <button type="button">Library</button>
          </nav>
          <main data-crosshook-focus-zone="content">
            <button type="button">Launch</button>
          </main>
          <aside data-crosshook-focus-zone="inspector">
            <button type="button">Inspector Action</button>
          </aside>
        </div>
      );
    }

    const onBack = vi.fn();
    render(<InspectorHarness onBack={onBack} />);

    const inspectorButton = screen.getByRole('button', { name: 'Inspector Action' });
    const contentRegion = screen.getByRole('main');

    inspectorButton.focus();
    expect(inspectorButton).toHaveFocus();

    act(() => {
      latestState?.back();
    });

    await waitFor(() => {
      expect(contentRegion.contains(document.activeElement)).toBe(true);
    });
    expect(onBack).not.toHaveBeenCalled();
  });

  it('focuses the content zone when the active sidebar route changes', async () => {
    render(<GamepadNavHarness enabled />);

    const sidebarButtons = screen
      .getAllByRole('button', { hidden: false })
      .filter((button) => ['Library', 'Profiles'].includes(button.textContent ?? ''));
    const libraryButton = sidebarButtons[0];
    const profilesButton = sidebarButtons[1];
    const contentRegion = screen.getByRole('main');

    libraryButton.focus();
    expect(libraryButton).toHaveFocus();

    act(() => {
      libraryButton.removeAttribute('data-state');
      profilesButton.setAttribute('data-state', 'active');
    });

    await waitFor(() => {
      expect(contentRegion.contains(document.activeElement)).toBe(true);
    });
    expect(contentRegion.contains(latestState?.activeElement ?? null)).toBe(true);
  });
});
