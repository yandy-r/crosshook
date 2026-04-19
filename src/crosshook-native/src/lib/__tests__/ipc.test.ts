import { beforeEach, describe, expect, it, vi } from 'vitest';

describe('callCommand', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it('routes browser-mode commands through the webdev IPC bridge', async () => {
    vi.doUnmock('@/lib/ipc');
    vi.doMock('@/lib/runtime', () => ({
      isTauri: () => false,
      isBrowserDevUi: () => true,
    }));
    const runMockCommand = vi.fn(async () => ({ auto_load_last_profile: false }));
    vi.doMock('@/lib/ipc.dev', () => ({
      runMockCommand,
    }));

    const { callCommand } = await import('@/lib/ipc');

    const settings = await callCommand<Record<string, unknown>>('settings_load');

    expect(runMockCommand).toHaveBeenCalledTimes(1);
    expect(runMockCommand).toHaveBeenCalledWith('settings_load', undefined);
    expect(settings).toHaveProperty('auto_load_last_profile');
  });

  it('routes Tauri-mode commands through Tauri invoke API', async () => {
    vi.resetModules();
    vi.doUnmock('@/lib/ipc');
    vi.doMock('@/lib/runtime', () => ({
      isTauri: () => true,
      isBrowserDevUi: () => false,
    }));

    const mockInvoke = vi.fn(async () => ({ success: true }));
    vi.doMock('@tauri-apps/api/core', () => ({
      invoke: mockInvoke,
    }));

    const { callCommand } = await import('@/lib/ipc');

    const result = await callCommand<Record<string, unknown>>('test_command', { arg: 'value' });

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(mockInvoke).toHaveBeenCalledWith('test_command', { arg: 'value' });
    expect(result).toEqual({ success: true });
  });
});
