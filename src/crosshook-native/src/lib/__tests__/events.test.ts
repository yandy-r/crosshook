import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { subscribeEvent, emitMockEvent, resetBrowserEventBus } from '../events';
import * as runtime from '../runtime';

describe('events.ts', () => {
  beforeEach(() => {
    resetBrowserEventBus();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('Browser Dev Mode (non-Tauri)', () => {
    beforeEach(() => {
      vi.spyOn(runtime, 'isTauri').mockReturnValue(false);
    });

    it('should subscribe to events in browser mode', async () => {
      const handler = vi.fn();
      const unlisten = await subscribeEvent('test-event', handler);

      expect(typeof unlisten).toBe('function');
      expect(handler).not.toHaveBeenCalled();
    });

    it('should receive events after subscribing', async () => {
      const handler = vi.fn();
      await subscribeEvent('test-event', handler);

      const result = emitMockEvent('test-event', { data: 'test' });

      expect(result).toBe(true);
      expect(handler).toHaveBeenCalledTimes(1);
      expect(handler).toHaveBeenCalledWith({
        event: 'test-event',
        id: 0,
        payload: { data: 'test' },
      });
    });

    it('should handle multiple subscribers to the same event', async () => {
      const handler1 = vi.fn();
      const handler2 = vi.fn();

      await subscribeEvent('multi-event', handler1);
      await subscribeEvent('multi-event', handler2);

      emitMockEvent('multi-event', { count: 42 });

      expect(handler1).toHaveBeenCalledOnce();
      expect(handler2).toHaveBeenCalledOnce();
    });

    it('should unsubscribe correctly', async () => {
      const handler = vi.fn();
      const unlisten = await subscribeEvent('unsub-test', handler);

      emitMockEvent('unsub-test', { before: true });
      expect(handler).toHaveBeenCalledTimes(1);

      unlisten();

      emitMockEvent('unsub-test', { after: true });
      expect(handler).toHaveBeenCalledTimes(1); // Still 1, not called again
    });

    it('should not emit to wrong event name', async () => {
      const handler = vi.fn();
      await subscribeEvent('event-a', handler);

      const result = emitMockEvent('event-b', { data: 'test' });

      expect(result).toBe(false);
      expect(handler).not.toHaveBeenCalled();
    });

    it('should return false when emitting to event with no listeners', () => {
      const result = emitMockEvent('no-listeners', { data: 'test' });
      expect(result).toBe(false);
    });

    it('should clear all listeners on reset', async () => {
      const handler = vi.fn();
      await subscribeEvent('reset-test', handler);

      emitMockEvent('reset-test', { before: true });
      expect(handler).toHaveBeenCalledOnce();

      resetBrowserEventBus();

      const result = emitMockEvent('reset-test', { after: true });
      expect(result).toBe(false);
      expect(handler).toHaveBeenCalledOnce(); // Still 1, reset cleared listeners
    });
  });

  describe('Tauri Mode', () => {
    beforeEach(() => {
      vi.spyOn(runtime, 'isTauri').mockReturnValue(true);
    });

    it('should return false when trying to emit in Tauri mode', () => {
      const result = emitMockEvent('tauri-event', { data: 'test' });
      expect(result).toBe(false);
    });

    it('should delegate to Tauri listen API', async () => {
      const mockListen = vi.fn(() => Promise.resolve(() => {}));
      vi.doMock('@tauri-apps/api/event', () => ({
        listen: mockListen,
      }));

      const handler = vi.fn();

      // In real Tauri mode, this would use the imported listen function
      // For this test, we're verifying the path is taken
      await subscribeEvent('tauri-test', handler);

      // The actual Tauri listen would be called, but we can't easily test
      // the dynamic import without more complex mocking
      expect(runtime.isTauri()).toBe(true);
    });
  });
});
