import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { isBrowserDevUi, isTauri } from '../runtime';

describe('runtime.ts', () => {
  const originalWindow = global.window;

  afterEach(() => {
    // Restore original window
    global.window = originalWindow;
  });

  describe('isTauri()', () => {
    it('should return true when __TAURI_INTERNALS__ is present', () => {
      // Mock window with Tauri internals
      global.window = {
        __TAURI_INTERNALS__: {},
      } as any;

      expect(isTauri()).toBe(true);
    });

    it('should return false when window is undefined', () => {
      // @ts-expect-error - testing undefined window
      global.window = undefined;

      expect(isTauri()).toBe(false);
    });

    it('should return false when __TAURI_INTERNALS__ is missing', () => {
      global.window = {} as any;

      expect(isTauri()).toBe(false);
    });
  });

  describe('isBrowserDevUi()', () => {
    it('should return true in plain browser context', () => {
      global.window = {} as any;

      expect(isBrowserDevUi()).toBe(true);
      expect(isTauri()).toBe(false);
    });

    it('should return false in Tauri context', () => {
      global.window = {
        __TAURI_INTERNALS__: {},
      } as any;

      expect(isBrowserDevUi()).toBe(false);
      expect(isTauri()).toBe(true);
    });

    it('should return false when window is undefined', () => {
      // @ts-expect-error - testing undefined window
      global.window = undefined;

      expect(isBrowserDevUi()).toBe(false);
    });
  });

  describe('Runtime detection consistency', () => {
    it('should be mutually exclusive', () => {
      // Test with Tauri
      global.window = { __TAURI_INTERNALS__: {} } as any;
      expect(isTauri() && isBrowserDevUi()).toBe(false);

      // Test with browser
      global.window = {} as any;
      expect(isTauri() && isBrowserDevUi()).toBe(false);
      expect(!isTauri() && isBrowserDevUi()).toBe(true);
    });
  });
});
