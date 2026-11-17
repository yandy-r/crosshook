import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { resetBenchWarning } from '../benchMark';
import { startBenchProbe } from '../benchProbe';
import { HEALTH_MIN_ROWS } from '../healthTable';
import { buildRoutes } from './healthFixture';

type Call = { name: string; args?: Record<string, unknown> };

const calls: Call[] = [];
const callCommandMock = vi.fn<(name: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: Record<string, unknown>) => callCommandMock(name, args),
}));

const marks = () => calls.filter((c) => c.name === 'bench_mark').map((c) => c.args as Record<string, unknown>);
const phases = () => marks().map((m) => `${m.phase}:${m.seq}`);
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

let frames: FrameRequestCallback[] = [];
function runFrames() {
  const pending = frames;
  frames = [];
  for (const cb of pending) {
    cb(0);
  }
}

/** Waits on wall time, not a flush count: under CPU load happy-dom delivers observer callbacks late. */
async function until(predicate: () => boolean, timeoutMs = 3000) {
  const deadline = Date.now() + timeoutMs;
  while (!predicate() && Date.now() < deadline) {
    await flush();
  }
  expect(predicate()).toBe(true);
}

/** Runs the probe's next frame once it is queued; under load the probe may queue it late. */
async function runNextFrame() {
  await until(() => frames.length > 0);
  runFrames();
}

function buildFavoriteDom() {
  document.body.innerHTML = `<button aria-pressed="false" aria-label="Toggle favorite: Game"></button>`;
  return document.querySelector('button') as HTMLButtonElement;
}

describe('startBenchProbe', () => {
  let controller: AbortController;

  beforeEach(() => {
    resetBenchWarning();
    calls.length = 0;
    frames = [];
    controller = new AbortController();
    callCommandMock.mockReset().mockImplementation(async (name, args) => {
      calls.push({ name, args });
      return name === 'bench_clock' ? 1 : undefined;
    });
    vi.stubGlobal('requestAnimationFrame', (cb: FrameRequestCallback) => {
      frames.push(cb);
      return frames.length;
    });
  });

  afterEach(() => {
    controller.abort();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    document.body.innerHTML = '';
  });

  it('syncs the clock, calls bench_begin, then arms and renders per trial with the observed time', async () => {
    const button = buildFavoriteDom();
    const probe = startBenchProbe('toggle_favorite', { signal: controller.signal, maxTrials: 2 });

    await until(() => phases().includes('armed:0'));
    const begin = calls.findIndex((c) => c.name === 'bench_begin');
    expect(begin).toBeGreaterThanOrEqual(20);
    expect(calls.slice(0, begin).every((c) => c.name === 'bench_clock')).toBe(true);
    expect(calls[begin + 1].name).toBe('bench_mark');
    expect(phases()).toEqual(['armed:0']);
    expect(document.activeElement).toBe(button);

    const before = performance.now();
    button.setAttribute('aria-pressed', 'true');
    await flush();
    expect(phases()).toEqual(['armed:0']); // render waits for the next frame
    expect(frames).toHaveLength(1);
    runFrames();
    await until(() => phases().includes('render:0'));

    const render = marks().find((m) => m.phase === 'render');
    expect(render?.scenario).toBe('toggle_favorite');
    expect(render?.tPerfMs as number).toBeGreaterThanOrEqual(before);
    expect(render?.tPerfMs as number).toBeLessThanOrEqual(performance.now());

    await until(() => phases().includes('armed:1'));
    button.setAttribute('aria-pressed', 'false');
    await flush();
    await runNextFrame();
    await probe;
    expect(phases()).toEqual(['armed:0', 'render:0', 'armed:1', 'render:1']);
  });

  it('does not re-send the render mark time from the frame callback', async () => {
    // dpad observes a synchronous focusin, so the change time is exact and no observer delivery is involved.
    document.body.innerHTML = `<main class="crosshook-focus-scope"><div class="crosshook-library-grid">
      <button class="crosshook-library-card__details-hitbox">A</button><button id="next">B</button></div></main>`;
    const probe = startBenchProbe('dpad', { signal: controller.signal, maxTrials: 1 });
    await until(() => phases().includes('armed:0'));
    (document.getElementById('next') as HTMLButtonElement).focus();
    const observed = performance.now();
    await new Promise((resolve) => setTimeout(resolve, 15));
    await runNextFrame();
    await probe;
    const render = marks().find((m) => m.phase === 'render');
    expect(render?.tPerfMs as number).toBeLessThanOrEqual(observed);
  });

  it('sends no mark and stops when the target is missing, so the runner times out on armed', async () => {
    await expect(startBenchProbe('toggle_favorite', { signal: controller.signal })).resolves.toBeUndefined();
    expect(phases()).toEqual([]);
  });

  it('calls bench_begin but sends no mark and does not throw when the scenario setup fails', async () => {
    vi.useFakeTimers();
    try {
      // toggle_optimization needs the library card's details button; without it prepare() times out.
      document.body.innerHTML = '<main></main>';
      const probe = startBenchProbe('toggle_optimization', { signal: controller.signal });
      await vi.advanceTimersByTimeAsync(11_000); // past the 10 s step timeout
      await expect(probe).resolves.toBeUndefined();
      expect(calls.some((c) => c.name === 'bench_begin')).toBe(true);
      expect(marks()).toEqual([]);
    } finally {
      vi.useRealTimers();
    }
  });

  it('dispatches replay_scroll to the replay probe: no clock sync, follow mark, then bench_begin', async () => {
    document.body.innerHTML = `
      <section data-testid="console-drawer" class="crosshook-console-drawer">
        <button aria-expanded="true"></button>
        <div class="crosshook-console__body"></div>
      </section>`;
    await startBenchProbe('replay_scroll', { signal: controller.signal });
    expect(calls.map((c) => c.name)).toEqual(['bench_mark', 'bench_begin']);
    expect(phases()).toEqual(['follow_on:0']);
    expect(marks()[0].scenario).toBe('replay_scroll');
  });

  it('dispatches health_scroll to the health probe: no clock sync, health_ready, then bench_begin', async () => {
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    vi.spyOn(window.HTMLElement.prototype, 'scrollIntoView').mockImplementation(() => {});
    await startBenchProbe('health_scroll', { signal: controller.signal });
    expect(calls.map((c) => c.name)).toEqual(['bench_mark', 'bench_begin']);
    expect(phases()).toEqual(['health_ready:0']);
    expect(marks()[0].scenario).toBe('health_scroll');
  });

  it('runs the scenario reset before every armed so each timed switch starts from the library', async () => {
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    const health = document.getElementById('r-trigger-health') as HTMLElement;
    const probe = startBenchProbe('health_table', { signal: controller.signal, maxTrials: 2 });
    await until(() => phases().includes('armed:0'));
    expect(document.getElementById('r-trigger-library')?.getAttribute('aria-selected')).toBe('true');
    expect(phases()[0]).toBe('health_ready:0'); // prepare reported the table before the first arm
    expect(document.activeElement).toBe(health);

    health.dispatchEvent(new MouseEvent('mousedown', { bubbles: true })); // the runner's Enter
    await flush();
    await runNextFrame();
    await until(() => phases().includes('render:0'));
    expect(document.getElementById('r-trigger-health')?.getAttribute('aria-selected')).toBe('true');

    await until(() => phases().includes('armed:1'));
    expect(document.getElementById('r-trigger-library')?.getAttribute('aria-selected')).toBe('true'); // reset ran
    controller.abort();
    await probe;
  });

  it('sends no mark and stops when a scenario reset fails', async () => {
    vi.useFakeTimers();
    try {
      buildRoutes({ healthRows: HEALTH_MIN_ROWS, selected: 'health' });
      document.getElementById('r-trigger-library')?.remove(); // reset cannot reach the library
      const probe = startBenchProbe('health_table', { signal: controller.signal });
      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(11_000);
      await expect(probe).resolves.toBeUndefined();
      expect(phases().some((p) => p.startsWith('armed'))).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it('does nothing for an unsupported scenario', async () => {
    vi.spyOn(console, 'warn').mockImplementation(() => {});
    await startBenchProbe('bogus', { signal: controller.signal });
    expect(calls).toHaveLength(0);
  });

  it('sends focus_lost on window blur with the current seq', async () => {
    buildFavoriteDom();
    const probe = startBenchProbe('toggle_favorite', { signal: controller.signal });
    await until(() => phases().includes('armed:0'));
    window.dispatchEvent(new Event('blur'));
    await until(() => phases().includes('focus_lost:0'));
    controller.abort();
    await probe;
  });

  it('removes its blur listener and trial observers when it ends', async () => {
    const button = buildFavoriteDom();
    const add = vi.spyOn(window, 'addEventListener');
    const remove = vi.spyOn(window, 'removeEventListener');
    const disconnect = vi.spyOn(MutationObserver.prototype, 'disconnect');
    const probe = startBenchProbe('toggle_favorite', { signal: controller.signal });
    await until(() => phases().includes('armed:0'));
    expect(add).toHaveBeenCalledWith('blur', expect.any(Function));

    controller.abort();
    await probe;
    const blur = add.mock.calls.find(([type]) => type === 'blur')?.[1];
    expect(remove).toHaveBeenCalledWith('blur', blur);
    expect(disconnect).toHaveBeenCalled();

    // Nothing is still listening: a later change sends no further marks.
    const count = calls.length;
    button.setAttribute('aria-pressed', 'true');
    window.dispatchEvent(new Event('blur'));
    await flush();
    expect(calls).toHaveLength(count);
  });

  it('stops at the trial cap', async () => {
    const button = buildFavoriteDom();
    const probe = startBenchProbe('toggle_favorite', { signal: controller.signal, maxTrials: 1 });
    await until(() => phases().includes('armed:0'));
    button.setAttribute('aria-pressed', 'true');
    await flush();
    await runNextFrame();
    await probe;
    expect(phases()).toEqual(['armed:0', 'render:0']);
  });

  it('adds no DOM nodes and never calls preventDefault', async () => {
    const button = buildFavoriteDom();
    const before = document.body.innerHTML;
    const prevent = vi.spyOn(Event.prototype, 'preventDefault');
    const probe = startBenchProbe('toggle_favorite', { signal: controller.signal, maxTrials: 1 });
    await until(() => phases().includes('armed:0'));
    button.setAttribute('aria-pressed', 'true');
    await flush();
    await runNextFrame();
    await probe;
    document.body.querySelector('button')?.setAttribute('aria-pressed', 'false');
    expect(document.body.children).toHaveLength(1);
    expect(document.body.innerHTML.replace('true', 'false')).toBe(before);
    expect(prevent).not.toHaveBeenCalled();
  });

  it('warns once and stops when bench_mark rejects', async () => {
    buildFavoriteDom();
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    callCommandMock.mockImplementation(async (name, args) => {
      calls.push({ name, args });
      if (name === 'bench_mark') {
        throw new Error('rejected');
      }
      return 1;
    });
    await startBenchProbe('toggle_favorite', { signal: controller.signal });
    expect(phases()).toEqual(['armed:0']);
    expect(warn.mock.calls.length).toBeLessThanOrEqual(1);
  });
});
