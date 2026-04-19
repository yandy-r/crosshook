import { beforeEach, describe, expect, it, vi } from 'vitest';

describe('callCommand', () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it('routes browser-mode commands through the webdev IPC bridge', async () => {
    vi.doUnmock('@/lib/ipc');
    vi.doUnmock('@/lib/ipc.dev');
    vi.doMock('@/lib/runtime', () => ({
      isTauri: () => false,
      isBrowserDevUi: () => true,
    }));

    const { callCommand } = await import('@/lib/ipc');

    const settings = await callCommand<Record<string, unknown>>('settings_load');

    expect(settings).toHaveProperty('auto_load_last_profile');
  });
});
