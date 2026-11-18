/** DOM builders shared by the health probe tests; they mirror the markup in `HealthDashboardPage.tsx`. */

export function healthTableHtml(label: string, rows: number, detailRows = 0): string {
  const body = Array.from({ length: rows }, (_, i) => `<tr class="crosshook-health-dashboard-row"><td>p${i}</td></tr>`);
  const detail = Array.from(
    { length: detailRows },
    () => '<tr class="crosshook-health-dashboard-detail"><td></td></tr>'
  );
  return `<table aria-label="${label}"><thead><tr><th>Name</th></tr></thead><tbody>${[...body, ...detail].join('')}</tbody></table>`;
}

const LIVE_LABEL = 'Profile health status';
export const CACHED_LABEL = 'Profile health status (cached)';

/** Appends a live table with `rows` profile rows (and optional detail rows) to `host`. */
export function appendLiveTable(host: Element, rows: number, detailRows = 0): void {
  host.insertAdjacentHTML('beforeend', healthTableHtml(LIVE_LABEL, rows, detailRows));
}

/**
 * Sidebar triggers plus a content root. Activating a trigger (mousedown) swaps the panel the way
 * `Tabs.Content key={route}` does, optionally rendering a live table of `healthRows` rows for health.
 */
export function buildRoutes(options: { healthRows: number; selected?: 'library' | 'health' }): HTMLElement {
  const { healthRows, selected = 'library' } = options;
  document.body.innerHTML = `
    <button role="tab" id="r-trigger-library" aria-selected="${selected === 'library'}"></button>
    <button role="tab" id="r-trigger-health" aria-selected="${selected === 'health'}"></button>
    <div data-crosshook-page-scroll="true"></div>`;
  const root = document.querySelector<HTMLElement>('[data-crosshook-page-scroll="true"]') as HTMLElement;
  const show = (route: 'library' | 'health') => {
    for (const value of ['library', 'health']) {
      document.getElementById(`r-trigger-${value}`)?.setAttribute('aria-selected', String(value === route));
    }
    root.innerHTML = `<div role="tabpanel" id="r-content-${route}"></div>`;
    if (route === 'health') {
      appendLiveTable(root.firstElementChild as Element, healthRows);
    }
  };
  for (const value of ['library', 'health'] as const) {
    document.getElementById(`r-trigger-${value}`)?.addEventListener('mousedown', () => show(value));
  }
  show(selected);
  return root;
}
