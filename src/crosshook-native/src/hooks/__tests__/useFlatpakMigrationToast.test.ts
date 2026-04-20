import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FlatpakMigrationCompletePayload } from '../useFlatpakMigrationToast';
import { FLATPAK_MIGRATION_TOAST_SESSION_KEY, useFlatpakMigrationToast } from '../useFlatpakMigrationToast';

type EventHandler = (event: { event: string; id: number; payload: FlatpakMigrationCompletePayload }) => void;

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

function emitMigrationEvent(payload: FlatpakMigrationCompletePayload): void {
  const handlers = capturedHandlers.get('flatpak-migration-complete') ?? [];
  for (const handler of handlers) {
    handler({ event: 'flatpak-migration-complete', id: 0, payload });
  }
}

const fullPayload: FlatpakMigrationCompletePayload = {
  imported_config: true,
  imported_subtrees: ['profiles', 'scripts'],
  skipped_subtrees: [],
};

const emptyPayload: FlatpakMigrationCompletePayload = {
  imported_config: false,
  imported_subtrees: [],
  skipped_subtrees: [],
};

describe('useFlatpakMigrationToast', () => {
  beforeEach(() => {
    capturedHandlers = new Map();
    mockUnlisten.mockReset();
    try {
      sessionStorage.removeItem(FLATPAK_MIGRATION_TOAST_SESSION_KEY);
    } catch {
      // ignore
    }
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it('unsubscribes on unmount', async () => {
    const { unmount } = renderHook(() => useFlatpakMigrationToast());

    // Allow the async subscribeEvent promise to resolve.
    await act(async () => {
      await Promise.resolve();
    });

    // Unmount triggers the effect cleanup; flush microtasks so the unlisten promise resolves.
    await act(async () => {
      unmount();
      await Promise.resolve();
    });

    expect(mockUnlisten).toHaveBeenCalledTimes(1);
  });

  it('dedup_via_sessionStorage — two events fired, toast shown once', async () => {
    const { result } = renderHook(() => useFlatpakMigrationToast());

    await act(async () => {
      await Promise.resolve();
    });

    // First event — should set importCount
    await act(async () => {
      emitMigrationEvent(fullPayload);
    });

    expect(result.current.importCount).toBe(3); // 2 subtrees + 1 config

    // Dismiss the toast (which also sets sessionStorage)
    act(() => {
      result.current.dismiss();
    });

    expect(result.current.importCount).toBeNull();

    // Second event — deduped via sessionStorage
    await act(async () => {
      emitMigrationEvent(fullPayload);
    });

    expect(result.current.importCount).toBeNull();
  });

  it('no-op when outcome is empty — imported_config=false, imported_subtrees=[]', async () => {
    const { result } = renderHook(() => useFlatpakMigrationToast());

    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      emitMigrationEvent(emptyPayload);
    });

    expect(result.current.importCount).toBeNull();
  });

  it('counts imported_subtrees.length + 1 when imported_config is true', async () => {
    const { result } = renderHook(() => useFlatpakMigrationToast());

    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      emitMigrationEvent({
        imported_config: true,
        imported_subtrees: ['profiles'],
        skipped_subtrees: [],
      });
    });

    expect(result.current.importCount).toBe(2); // 1 subtree + 1 config
  });

  it('counts only subtrees when imported_config is false', async () => {
    const { result } = renderHook(() => useFlatpakMigrationToast());

    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      emitMigrationEvent({
        imported_config: false,
        imported_subtrees: ['profiles', 'scripts', 'trainers'],
        skipped_subtrees: [],
      });
    });

    expect(result.current.importCount).toBe(3);
  });

  it('dismiss sets importCount to null', async () => {
    const { result } = renderHook(() => useFlatpakMigrationToast());

    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      emitMigrationEvent(fullPayload);
    });

    expect(result.current.importCount).not.toBeNull();

    act(() => {
      result.current.dismiss();
    });

    expect(result.current.importCount).toBeNull();
  });
});
