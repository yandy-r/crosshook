import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resetBenchWarning, sendMark, warnOnce } from '../benchMark';

const callCommandMock = vi.fn<(name: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: Record<string, unknown>) => callCommandMock(name, args),
}));

describe('sendMark', () => {
  beforeEach(() => {
    resetBenchWarning();
    callCommandMock.mockReset().mockResolvedValue(undefined);
    vi.restoreAllMocks();
  });

  it('forwards the payload to bench_mark and resolves true', async () => {
    await expect(sendMark('health_table', 3, 'health_ready', 12.5)).resolves.toBe(true);
    expect(callCommandMock).toHaveBeenCalledWith('bench_mark', {
      scenario: 'health_table',
      seq: 3,
      phase: 'health_ready',
      tPerfMs: 12.5,
    });
  });

  it('resolves false instead of rejecting and warns once across failures', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    callCommandMock.mockRejectedValue(new Error('rejected'));
    await expect(sendMark('replay_scroll', 0, 'follow_on', 1)).resolves.toBe(false);
    await expect(sendMark('replay_scroll', 0, 'follow_off', 2)).resolves.toBe(false);
    expect(warn).toHaveBeenCalledTimes(1);
  });

  it('shares the warn-once with warnOnce callers and can be re-armed', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    warnOnce('first', 1);
    warnOnce('second', 2);
    expect(warn).toHaveBeenCalledTimes(1);
    resetBenchWarning();
    warnOnce('third', 3);
    expect(warn).toHaveBeenCalledTimes(2);
  });
});
