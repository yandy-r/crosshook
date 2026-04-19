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
});
