import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { emitMockEvent, resetBrowserEventBus } from '@/lib/events';
import * as runtime from '@/lib/runtime';
import {
  UI_SETTINGS_PERSIST_FAILED_EVENT,
  UI_SETTINGS_PERSIST_FAILED_MESSAGE,
  useUiSettingsPersistFailedToast,
} from '../useUiSettingsPersistFailedToast';

function ToastProbe() {
  const { detailMessage, dismiss } = useUiSettingsPersistFailedToast();
  if (detailMessage === null) {
    return null;
  }
  return (
    <div className="crosshook-status-toast" role="status" aria-live="polite">
      <span title={detailMessage}>{UI_SETTINGS_PERSIST_FAILED_MESSAGE}</span>
      <button type="button" aria-label="Dismiss" onClick={dismiss}>
        ×
      </button>
    </div>
  );
}

describe('useUiSettingsPersistFailedToast', () => {
  beforeEach(() => {
    resetBrowserEventBus();
    vi.spyOn(runtime, 'isTauri').mockReturnValue(false);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('shows one toast per session even when the event fires twice', async () => {
    render(<ToastProbe />);

    // Wait until the subscription is registered before emitting.
    await act(async () => {});
    act(() => {
      emitMockEvent(UI_SETTINGS_PERSIST_FAILED_EVENT, { message: 'persist boom' });
      emitMockEvent(UI_SETTINGS_PERSIST_FAILED_EVENT, { message: 'persist boom again' });
    });

    await waitFor(() => {
      expect(screen.getAllByRole('status')).toHaveLength(1);
    });
    const message = screen.getByText(UI_SETTINGS_PERSIST_FAILED_MESSAGE);
    expect(message).toHaveAttribute('title', 'persist boom');
  });

  it('stays hidden when no failure event fires', async () => {
    render(<ToastProbe />);
    await act(async () => {});
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('does not re-show the toast after dismiss when the event fires again', async () => {
    const user = userEvent.setup();
    render(<ToastProbe />);
    await act(async () => {});
    act(() => {
      emitMockEvent(UI_SETTINGS_PERSIST_FAILED_EVENT, { message: 'persist boom' });
    });
    await waitFor(() => {
      expect(screen.getByRole('status')).toBeInTheDocument();
    });

    await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(screen.queryByRole('status')).toBeNull();

    act(() => {
      emitMockEvent(UI_SETTINGS_PERSIST_FAILED_EVENT, { message: 'persist boom again' });
    });
    await act(async () => {});
    expect(screen.queryByRole('status')).toBeNull();
  });

  it('unsubscribes on unmount', async () => {
    const { unmount } = render(<ToastProbe />);
    await act(async () => {});
    unmount();
    expect(emitMockEvent(UI_SETTINGS_PERSIST_FAILED_EVENT, { message: 'persist boom' })).toBe(false);
  });
});
