import { act, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { type GamepadNavState, useGamepadNav } from '../useGamepadNav';

interface HarnessProps {
  enabled?: boolean;
  onBack?: () => void;
}

let latestState: GamepadNavState | null = null;

function GamepadNavHarness({ enabled = false, onBack }: HarnessProps) {
  const nav = useGamepadNav({ enabled, onBack });
  latestState = nav;

  return (
    <div ref={(node) => (nav.rootRef.current = node)} data-testid="nav-root">
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
