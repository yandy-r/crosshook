import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProfileSummary } from '@/types/library';
import { useProfileSummaries } from '../useProfileSummaries';

type EventHandler = (event: { event: string; id: number; payload: string }) => void;

const mockUnlisten = vi.fn();
let capturedHandlers: Map<string, EventHandler[]>;

vi.mock('@/lib/events', () => ({
  subscribeEvent: vi.fn((name: string, handler: EventHandler): Promise<() => void> => {
    const handlers = capturedHandlers.get(name) ?? [];
    handlers.push(handler);
    capturedHandlers.set(name, handlers);
    return Promise.resolve(mockUnlisten);
  }),
}));

const initialSummaries: ProfileSummary[] = [
  {
    name: 'Synthetic Quest',
    gameName: 'Synthetic Quest',
    steamAppId: '9999001',
    networkIsolation: false,
  },
];

const updatedSummaries: ProfileSummary[] = [
  ...initialSummaries,
  {
    name: 'New Profile',
    gameName: 'New Game',
    steamAppId: '9999002',
    networkIsolation: false,
  },
];

let commandResponses: Map<string, () => Promise<unknown>>;

vi.mock('@/lib/ipc', () => ({
  callCommand: vi.fn((name: string) => {
    const handler = commandResponses.get(name);
    if (!handler) {
      throw new Error(`[test-mock] Unhandled command: ${name}`);
    }
    return handler();
  }),
}));

function emitProfilesChanged(): void {
  const handlers = capturedHandlers.get('profiles-changed') ?? [];
  for (const handler of handlers) {
    handler({ event: 'profiles-changed', id: 0, payload: '' });
  }
}

describe('useProfileSummaries', () => {
  beforeEach(() => {
    capturedHandlers = new Map();
    mockUnlisten.mockReset();
    commandResponses = new Map();
    commandResponses.set('profile_list_summaries', async () => initialSummaries);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('initial fetch populates summaries', async () => {
    const { result } = renderHook(() => useProfileSummaries([]));

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.summaries).toEqual(initialSummaries);
    expect(result.current.error).toBeNull();
  });

  it('emitting profiles-changed triggers a refetch and updates summaries', async () => {
    const { result } = renderHook(() => useProfileSummaries([]));

    // Wait for initial fetch
    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    expect(result.current.summaries).toEqual(initialSummaries);

    // Switch to updated response before firing event
    commandResponses.set('profile_list_summaries', async () => updatedSummaries);

    // Allow subscribeEvent promise to resolve so handler is registered
    await act(async () => {
      await Promise.resolve();
    });

    // Emit the event
    await act(async () => {
      emitProfilesChanged();
    });

    await waitFor(() => {
      expect(result.current.summaries).toEqual(updatedSummaries);
    });
  });

  it('calls unsubscribe on unmount', async () => {
    const { unmount } = renderHook(() => useProfileSummaries([]));

    // Allow the async subscribeEvent promise to resolve
    await act(async () => {
      await Promise.resolve();
    });

    // Unmount triggers effect cleanup; flush microtasks so the unlisten promise resolves
    await act(async () => {
      unmount();
      await Promise.resolve();
    });

    expect(mockUnlisten).toHaveBeenCalledTimes(1);
  });
});
