import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resetBenchWarning } from '../benchMark';
import { CONSOLE_WAIT_TIMEOUT_MS, startReplayProbe } from '../replayProbe';

type Call = { name: string; args?: Record<string, unknown> };

const calls: Call[] = [];
const callCommandMock = vi.fn<(name: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: Record<string, unknown>) => callCommandMock(name, args),
}));

const names = () => calls.map((c) => c.name);
const marks = () => calls.filter((c) => c.name === 'bench_mark').map((c) => c.args as Record<string, unknown>);
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

interface Geometry {
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;
}

function buildDrawer({
  collapsed = false,
  body = true,
  geometry,
}: {
  collapsed?: boolean;
  body?: boolean;
  geometry?: Geometry;
}) {
  document.body.innerHTML = `
    <section data-testid="console-drawer" class="crosshook-console-drawer${collapsed ? ' crosshook-console-drawer--collapsed' : ''}">
      <button type="button" aria-expanded="${!collapsed}"></button>
      <div class="crosshook-console-drawer__body">
        <section class="crosshook-console">
          ${body ? '<div class="crosshook-console__body"></div>' : ''}
        </section>
      </div>
    </section>`;
  const element = document.querySelector<HTMLElement>('.crosshook-console__body');
  if (element && geometry) {
    for (const [key, value] of Object.entries(geometry)) {
      Object.defineProperty(element, key, { value, configurable: true });
    }
  }
  return element;
}

function expand() {
  const drawer = document.querySelector('[data-testid="console-drawer"]') as HTMLElement;
  drawer.classList.remove('crosshook-console-drawer--collapsed');
  drawer.querySelector('button')?.setAttribute('aria-expanded', 'true');
}

describe('startReplayProbe', () => {
  beforeEach(() => {
    resetBenchWarning();
    calls.length = 0;
    callCommandMock.mockReset().mockImplementation(async (name, args) => {
      calls.push({ name, args });
      return undefined;
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    document.body.innerHTML = '';
  });

  it('sends follow_on at seq 0, then bench_begin, with no clock sync', async () => {
    buildDrawer({ geometry: { scrollHeight: 1000, scrollTop: 600, clientHeight: 400 } });
    await startReplayProbe();
    expect(names()).toEqual(['bench_mark', 'bench_begin']);
    expect(marks()[0]).toMatchObject({ scenario: 'replay_scroll', seq: 0, phase: 'follow_on' });
    expect(typeof marks()[0].tPerfMs).toBe('number');
    expect(calls[1].args).toBeUndefined();
  });

  it('treats a distance of exactly 24 px as following and 25 px as not', async () => {
    buildDrawer({ geometry: { scrollHeight: 1000, scrollTop: 576, clientHeight: 400 } });
    await startReplayProbe();
    expect(marks()[0].phase).toBe('follow_on');

    calls.length = 0;
    buildDrawer({ geometry: { scrollHeight: 1000, scrollTop: 575, clientHeight: 400 } });
    await startReplayProbe();
    expect(marks()[0].phase).toBe('follow_off');
    expect(names()).toEqual(['bench_mark', 'bench_begin']);
  });

  it('sends console_hidden and never calls bench_begin when there is no drawer (status-bar layout)', async () => {
    vi.useFakeTimers();
    document.body.innerHTML = '<section data-testid="console-status-bar"></section>';
    const probe = startReplayProbe();
    await vi.advanceTimersByTimeAsync(CONSOLE_WAIT_TIMEOUT_MS + 1);
    await expect(probe).resolves.toBeUndefined();
    expect(names()).toEqual(['bench_mark']);
    expect(marks()[0]).toMatchObject({ scenario: 'replay_scroll', seq: 0, phase: 'console_hidden' });
  });

  it('sends console_hidden when the drawer stays collapsed', async () => {
    vi.useFakeTimers();
    buildDrawer({ collapsed: true });
    const probe = startReplayProbe({ timeoutMs: 1000 });
    await vi.advanceTimersByTimeAsync(1001);
    await probe;
    expect(names()).toEqual(['bench_mark']);
    expect(marks()[0].phase).toBe('console_hidden');
  });

  it('does not report console_hidden before the timeout', async () => {
    vi.useFakeTimers();
    buildDrawer({ collapsed: true });
    const probe = startReplayProbe();
    await vi.advanceTimersByTimeAsync(CONSOLE_WAIT_TIMEOUT_MS - 1);
    expect(calls).toEqual([]);
    await vi.advanceTimersByTimeAsync(2);
    await probe;
    expect(marks()[0].phase).toBe('console_hidden');
  });

  it('waits for a collapsed drawer to be expanded and proceeds', async () => {
    buildDrawer({ collapsed: true });
    const probe = startReplayProbe();
    await flush();
    expect(calls).toEqual([]);
    expand();
    await probe;
    expect(names()).toEqual(['bench_mark', 'bench_begin']);
    expect(marks()[0].phase).toBe('follow_on');
  });

  it('requires the console body, not just an expanded drawer', async () => {
    vi.useFakeTimers();
    buildDrawer({ body: false });
    const probe = startReplayProbe({ timeoutMs: 500 });
    await vi.advanceTimersByTimeAsync(501);
    await probe;
    expect(marks().map((m) => m.phase)).toEqual(['console_hidden']);
    expect(names()).not.toContain('bench_begin');
  });

  it('stops without a mark when aborted while waiting', async () => {
    buildDrawer({ collapsed: true });
    const controller = new AbortController();
    const probe = startReplayProbe({ signal: controller.signal });
    controller.abort();
    await probe;
    expect(calls).toEqual([]);
  });

  it('does not call bench_begin and does not throw when the follow mark rejects', async () => {
    buildDrawer({});
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    callCommandMock.mockImplementation(async (name, args) => {
      calls.push({ name, args });
      throw new Error('rejected');
    });
    await expect(startReplayProbe()).resolves.toBeUndefined();
    expect(names()).toEqual(['bench_mark']);
    expect(warn).toHaveBeenCalled();
  });

  it('does not throw when bench_begin rejects', async () => {
    buildDrawer({});
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    callCommandMock.mockImplementation(async (name, args) => {
      calls.push({ name, args });
      if (name === 'bench_begin') {
        throw new Error('rejected');
      }
      return undefined;
    });
    await expect(startReplayProbe()).resolves.toBeUndefined();
    expect(names()).toEqual(['bench_mark', 'bench_begin']);
  });

  describe('focus_lost witness', () => {
    const blur = () => window.dispatchEvent(new Event('blur'));
    const focusLost = () => marks().filter((m) => m.phase === 'focus_lost');

    it('sends focus_lost at seq 0 for a blur after bench_begin, once the probe has returned', async () => {
      buildDrawer({});
      await startReplayProbe();
      expect(names()).toEqual(['bench_mark', 'bench_begin']);
      blur();
      await flush();
      expect(focusLost()).toHaveLength(1);
      expect(focusLost()[0]).toMatchObject({ scenario: 'replay_scroll', seq: 0, phase: 'focus_lost' });
      expect(typeof focusLost()[0].tPerfMs).toBe('number');
    });

    it('registers a persistent (not once) blur listener', async () => {
      buildDrawer({});
      const add = vi.spyOn(window, 'addEventListener');
      await startReplayProbe();
      const registration = add.mock.calls.find(([type]) => type === 'blur');
      expect(registration?.[2]).toEqual({ once: false });
    });

    it('sends at most one focus_lost however many blurs follow', async () => {
      buildDrawer({});
      await startReplayProbe();
      blur();
      blur();
      blur();
      await flush();
      expect(focusLost()).toHaveLength(1);
    });

    it('sends nothing without a blur', async () => {
      buildDrawer({});
      await startReplayProbe();
      await flush();
      expect(focusLost()).toEqual([]);
    });

    it('also witnesses a blur while the console is still being awaited', async () => {
      buildDrawer({ collapsed: true });
      const probe = startReplayProbe();
      blur();
      await flush();
      expect(marks().map((m) => m.phase)).toEqual(['focus_lost']);
      expand();
      await probe;
      expect(names()).toEqual(['bench_mark', 'bench_mark', 'bench_begin']);
    });

    it('does not register a second listener when the probe starts twice', async () => {
      buildDrawer({});
      await startReplayProbe();
      await startReplayProbe();
      blur();
      await flush();
      expect(focusLost()).toHaveLength(1);
    });

    it('is removed by an abort, even after bench_begin', async () => {
      buildDrawer({});
      const controller = new AbortController();
      await startReplayProbe({ signal: controller.signal });
      controller.abort();
      blur();
      await flush();
      expect(focusLost()).toEqual([]);
    });

    it('is not registered when the signal is already aborted', async () => {
      buildDrawer({});
      const controller = new AbortController();
      controller.abort();
      await startReplayProbe({ signal: controller.signal });
      blur();
      await flush();
      expect(calls).toEqual([]);
    });

    it('does not throw when the focus_lost mark is rejected', async () => {
      buildDrawer({});
      await startReplayProbe();
      const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
      callCommandMock.mockImplementation(async () => {
        throw new Error('rejected');
      });
      blur();
      await flush();
      expect(warn).toHaveBeenCalled();
    });
  });

  it('adds no DOM nodes and removes its observer', async () => {
    buildDrawer({});
    const before = document.body.innerHTML;
    await startReplayProbe();
    expect(document.body.innerHTML).toBe(before);
  });
});
