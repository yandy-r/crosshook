import { callCommand } from '@/lib/ipc';
import type { BenchMarkPhase } from './benchProbe';

/** Scenario name the runner passes for the G8 session replay. */
export const REPLAY_SCENARIO = 'replay_scroll';
/** How long the probe waits for the console drawer to be expanded with its log body. */
export const CONSOLE_WAIT_TIMEOUT_MS = 30_000;
/** Same follow threshold as `ConsoleView.updateFollowState`. */
export const FOLLOW_THRESHOLD_PX = 24;

const DRAWER = '[data-testid="console-drawer"]';
const DRAWER_COLLAPSED_CLASS = 'crosshook-console-drawer--collapsed';
const DRAWER_TOGGLE = 'button[aria-expanded]';
const CONSOLE_BODY = '.crosshook-console__body';

export interface ReplayProbeOptions {
  /** Stops the wait (tests; production never passes one). */
  signal?: AbortSignal;
  /** Overrides `CONSOLE_WAIT_TIMEOUT_MS` (tests). */
  timeoutMs?: number;
}

/**
 * The drawer layout renders `ConsoleDrawer` with `aria-expanded` on its toggle button and the
 * `--collapsed` class on the section; the status-bar layout has no `console-drawer` at all.
 * The body is in the DOM even while collapsed, so both the class and the toggle are checked.
 */
function findVisibleConsoleBody(): HTMLElement | null {
  const drawer = document.querySelector(DRAWER);
  if (!drawer || drawer.classList.contains(DRAWER_COLLAPSED_CLASS)) {
    return null;
  }
  if (drawer.querySelector(DRAWER_TOGGLE)?.getAttribute('aria-expanded') !== 'true') {
    return null;
  }
  return drawer.querySelector<HTMLElement>(CONSOLE_BODY);
}

function waitForConsoleBody(timeoutMs: number, signal?: AbortSignal): Promise<HTMLElement | null> {
  const existing = findVisibleConsoleBody();
  if (existing || signal?.aborted) {
    return Promise.resolve(existing);
  }
  return new Promise((resolve) => {
    const finish = (found: HTMLElement | null) => {
      observer.disconnect();
      clearTimeout(timer);
      signal?.removeEventListener('abort', onAbort);
      resolve(found);
    };
    const onAbort = () => finish(null);
    const observer = new MutationObserver(() => {
      const found = findVisibleConsoleBody();
      if (found) {
        finish(found);
      }
    });
    const timer = setTimeout(() => finish(null), timeoutMs);
    signal?.addEventListener('abort', onAbort, { once: true });
    observer.observe(document.body, {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ['class', 'aria-expanded'],
    });
  });
}

function isFollowing(body: HTMLElement): boolean {
  return body.scrollHeight - (body.scrollTop + body.clientHeight) <= FOLLOW_THRESHOLD_PX;
}

async function sendMark(phase: BenchMarkPhase): Promise<boolean> {
  try {
    await callCommand('bench_mark', { scenario: REPLAY_SCENARIO, seq: 0, phase, tPerfMs: performance.now() });
    return true;
  } catch (error) {
    console.warn('bench replay probe: bench_mark failed', error);
    return false;
  }
}

/** Removes the blur witness the previous `startReplayProbe` registered, so a second start never doubles it. */
let removeBlurWitness: (() => void) | null = null;

/**
 * Sends one `focus_lost` mark (seq 0) at the first window blur and then stays quiet. The listener
 * is not `once`: it stays registered for the life of the page, past `bench_begin`, because the
 * runner needs the witness while the replay scrolls. Only an abort removes it.
 */
function watchFocusLoss(signal?: AbortSignal): void {
  removeBlurWitness?.();
  removeBlurWitness = null;
  if (signal?.aborted) {
    return;
  }
  let sent = false;
  const onBlur = () => {
    if (sent) {
      return;
    }
    sent = true;
    void sendMark('focus_lost');
  };
  const remove = () => {
    window.removeEventListener('blur', onBlur);
    signal?.removeEventListener('abort', remove);
    if (removeBlurWitness === remove) {
      removeBlurWitness = null;
    }
  };
  window.addEventListener('blur', onBlur, { once: false });
  signal?.addEventListener('abort', remove, { once: true });
  removeBlurWitness = remove;
}

/**
 * Bench-only probe for the G8 session replay. Does no clock sync (the replay marks come from the
 * backend). Waits for the console log to be visible, reports `console_hidden` when it is not, and
 * otherwise reports the follow state (`follow_on`/`follow_off`) and calls `bench_begin`, which
 * releases the replay once the runner says go. A window blur at any point after the call sends
 * `focus_lost` (once); the listener outlives the returned promise. Never rejects.
 */
export async function startReplayProbe(options: ReplayProbeOptions = {}): Promise<void> {
  const { signal, timeoutMs = CONSOLE_WAIT_TIMEOUT_MS } = options;
  watchFocusLoss(signal);
  try {
    const body = await waitForConsoleBody(timeoutMs, signal);
    if (signal?.aborted) {
      return;
    }
    if (!body) {
      await sendMark('console_hidden');
      return;
    }
    if (!(await sendMark(isFollowing(body) ? 'follow_on' : 'follow_off'))) {
      return;
    }
    await callCommand('bench_begin');
  } catch (error) {
    console.warn('bench replay probe failed:', error);
  }
}
