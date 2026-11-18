/**
 * Bench-only helpers for the health dashboard supplementals (G5 `health_table`, G4 `health`).
 * They read the DOM only: no nodes or styles are added and every observer is disconnected.
 * Reachable only through the dynamic import in `useBenchReady`, so production never loads them.
 */

import { activate, tabTrigger, waitForElement } from './benchDom';
import { sendMark } from './benchMark';

/** App scenario the runner passes for the G5 health-table latency. */
export const HEALTH_TABLE_SCENARIO = 'health_table';
/** Fewer live rows than this at the timeout is `health_rows_too_few`; mirrors `bench_events.HEALTH_MIN_ROWS`. */
export const HEALTH_MIN_ROWS = 300;
/** How long to wait for the live table to reach `HEALTH_MIN_ROWS`; mirrors `bench_events.HEALTH_WAIT_S`. */
export const HEALTH_WAIT_MS = 60_000;

/** Exact `aria-label` match: the cached table is labelled "Profile health status (cached)" and is excluded. */
export const LIVE_TABLE = 'table[aria-label="Profile health status"]';
/** Profile rows only: the expanded detail rows do not carry this class. */
const ROW = 'tr.crosshook-health-dashboard-row';
/** The route content root that `Tabs.Content key={route}` remounts on every switch. */
export const CONTENT_ROOT = '[data-crosshook-page-scroll="true"]';
export const LIBRARY_CONTENT = '.crosshook-library-page__content';

const ROUTE_WAIT_TIMEOUT_MS = 10_000;

/** Counts profile rows in the live health table's first `tbody` (0 when the table is absent). */
export function liveHealthRowCount(root: ParentNode = document): number {
  const table = root.querySelector<HTMLTableElement>(LIVE_TABLE);
  return table?.tBodies[0]?.querySelectorAll(ROW).length ?? 0;
}

/**
 * Resolves the live row count as soon as it reaches `HEALTH_MIN_ROWS`, or the last count seen at
 * `timeoutMs` or when `signal` aborts. The cached table never counts.
 */
export function waitForHealthRows(timeoutMs: number, signal?: AbortSignal): Promise<number> {
  const initial = liveHealthRowCount();
  if (initial >= HEALTH_MIN_ROWS || signal?.aborted) {
    return Promise.resolve(initial);
  }
  return new Promise((resolve) => {
    let last = initial;
    const finish = () => {
      observer.disconnect();
      clearTimeout(timer);
      signal?.removeEventListener('abort', finish);
      resolve(last);
    };
    const observer = new MutationObserver(() => {
      last = liveHealthRowCount();
      if (last >= HEALTH_MIN_ROWS) {
        finish();
      }
    });
    const timer = setTimeout(() => {
      last = liveHealthRowCount();
      finish();
    }, timeoutMs);
    signal?.addEventListener('abort', finish, { once: true });
    observer.observe(document.body, { childList: true, subtree: true });
  });
}

/** Activates the Sidebar trigger for `value` and waits for that route's panel. False when either is missing. */
export async function openRoute(value: string, signal?: AbortSignal): Promise<boolean> {
  const trigger = tabTrigger(value);
  if (!trigger) {
    return false;
  }
  activate(trigger);
  const panel = await waitForElement(`[role="tabpanel"][id$="-content-${value}"]`, ROUTE_WAIT_TIMEOUT_MS, signal);
  return panel !== null;
}

/**
 * Opens the health route, waits for the live table, and sends `health_ready` or
 * `health_rows_too_few` (seq 0). Returns true only when the table was ready and the mark was sent.
 * A missing trigger or an abort sends no mark, so the runner reports the missing status instead.
 */
export async function reportHealthTable(
  scenario: string,
  signal?: AbortSignal,
  timeoutMs: number = HEALTH_WAIT_MS
): Promise<boolean> {
  if (!(await openRoute('health', signal)) || signal?.aborted) {
    return false;
  }
  const rows = await waitForHealthRows(timeoutMs, signal);
  if (signal?.aborted) {
    return false;
  }
  const ready = rows >= HEALTH_MIN_ROWS;
  const sent = await sendMark(scenario, 0, ready ? 'health_ready' : 'health_rows_too_few', performance.now());
  return ready && sent;
}
