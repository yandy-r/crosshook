import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getScenario, type ScenarioTrial } from '../benchScenarios';
import { HEALTH_MIN_ROWS } from '../healthTable';
import { appendLiveTable, buildRoutes, CACHED_LABEL, healthTableHtml } from './healthFixture';

const callCommandMock = vi.fn<(name: string, args?: Record<string, unknown>) => Promise<unknown>>();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: Record<string, unknown>) => callCommandMock(name, args),
}));

const trials: ScenarioTrial[] = [];

function arm(name: string): ScenarioTrial | null {
  const trial = getScenario(name)?.arm() ?? null;
  if (trial) {
    trials.push(trial);
  }
  return trial;
}

async function settle(trial: ScenarioTrial): Promise<number | 'pending'> {
  return Promise.race([trial.changed, new Promise<'pending'>((resolve) => setTimeout(() => resolve('pending'), 20))]);
}

beforeEach(() => {
  callCommandMock.mockReset().mockResolvedValue(undefined);
});

afterEach(() => {
  trials.splice(0).forEach((trial) => {
    trial.dispose();
  });
  document.body.innerHTML = '';
});

describe('getScenario', () => {
  it('returns null for unsupported or inherited names', () => {
    expect(getScenario('nope')).toBeNull();
    expect(getScenario('toString')).toBeNull();
    expect(getScenario('search')).not.toBeNull();
  });
});

describe('scenario arm() with a missing target', () => {
  it.each([
    'search',
    'toggle_favorite',
    'toggle_optimization',
    'dpad',
    'route_switch',
    'health_table',
  ])('%s returns null', (name) => {
    expect(() => arm(name)).not.toThrow();
    expect(arm(name)).toBeNull();
  });
});

describe('search', () => {
  function build() {
    document.body.innerHTML = `
      <div class="crosshook-library-page__content">
        <div class="crosshook-library-toolbar"><input type="search" aria-label="Search games" /></div>
        <ul class="crosshook-library-grid"><li id="c1"></li></ul>
      </div>`;
    return document.querySelector<HTMLInputElement>('input[aria-label="Search games"]') as HTMLInputElement;
  }

  it('focuses the input and fires on a grid mutation after the value changed', async () => {
    const input = build();
    const trial = arm('search') as ScenarioTrial;
    expect(document.activeElement).toBe(input);

    input.value = 'a';
    document.querySelector('.crosshook-library-grid')?.appendChild(document.createElement('li'));
    expect(typeof (await settle(trial))).toBe('number');
  });

  it('ignores a toolbar-only mutation even when the value changed', async () => {
    const input = build();
    const trial = arm('search') as ScenarioTrial;
    input.value = 'a';
    document.querySelector('.crosshook-library-toolbar')?.appendChild(document.createElement('span'));
    expect(await settle(trial)).toBe('pending');
  });

  it('ignores grid mutations while the value is unchanged', async () => {
    build();
    const trial = arm('search') as ScenarioTrial;
    document.querySelector('.crosshook-library-grid')?.appendChild(document.createElement('li'));
    expect(await settle(trial)).toBe('pending');
  });
});

describe('toggle_favorite', () => {
  it('fires when aria-pressed flips and ignores unrelated attributes', async () => {
    document.body.innerHTML = `<button aria-pressed="false" aria-label="Toggle favorite: Game"></button>`;
    const button = document.querySelector('button') as HTMLButtonElement;
    const trial = arm('toggle_favorite') as ScenarioTrial;
    expect(document.activeElement).toBe(button);

    button.setAttribute('data-x', '1');
    expect(await settle(trial)).toBe('pending');
    button.setAttribute('aria-pressed', 'true');
    expect(typeof (await settle(trial))).toBe('number');
  });
});

describe('toggle_optimization', () => {
  it('fires on the checkbox change event and removes the listener on dispose', async () => {
    document.body.innerHTML = `
      <div role="tabpanel" id="x-content-optimizations">
        <input type="checkbox" class="crosshook-launch-optimizations__checkbox" disabled />
        <input type="checkbox" class="crosshook-launch-optimizations__checkbox" />
      </div>`;
    const checkbox = document.querySelectorAll<HTMLInputElement>('input')[1];
    const removeSpy = vi.spyOn(checkbox, 'removeEventListener');

    const trial = arm('toggle_optimization') as ScenarioTrial;
    expect(document.activeElement).toBe(checkbox);
    checkbox.dispatchEvent(new Event('change', { bubbles: true }));
    expect(typeof (await settle(trial))).toBe('number');
    trial.dispose();
    expect(removeSpy).toHaveBeenCalledWith('change', expect.any(Function));
  });

  it('prepare walks details -> hero tab -> optimizations sub-tab', async () => {
    document.body.innerHTML = `<button class="crosshook-library-card__btn--details"></button>`;
    const clicks: string[] = [];
    const heroTab = document.createElement('button');
    heroTab.setAttribute('role', 'tab');
    heroTab.id = 'a-trigger-launch-options';
    const subTab = document.createElement('button');
    subTab.setAttribute('role', 'tab');
    subTab.id = 'b-trigger-optimizations';
    const panel = document.createElement('div');
    panel.setAttribute('role', 'tabpanel');
    panel.id = 'b-content-optimizations';
    panel.innerHTML = '<input type="checkbox" class="crosshook-launch-optimizations__checkbox" />';

    document.querySelector('button')?.addEventListener('click', () => {
      clicks.push('details');
      document.body.append(heroTab);
    });
    heroTab.addEventListener('mousedown', () => {
      clicks.push('hero');
      document.body.append(subTab);
    });
    subTab.addEventListener('mousedown', () => {
      clicks.push('sub');
      document.body.append(panel);
    });

    await expect(getScenario('toggle_optimization')?.prepare?.()).resolves.toBe(true);
    expect(clicks).toEqual(['details', 'hero', 'sub']);
  });
});

describe('dpad', () => {
  function build() {
    document.body.innerHTML = `
      <main class="crosshook-focus-scope">
        <ul class="crosshook-library-grid"><li><button class="crosshook-library-card__details-hitbox" id="c1"></button><button id="c2"></button></li></ul>
      </main>
      <button id="outside"></button>`;
  }

  it('fires on focusin of a different element inside the gamepad root only', async () => {
    build();
    const trial = arm('dpad') as ScenarioTrial;
    expect(document.activeElement?.id).toBe('c1');

    document.getElementById('outside')?.focus();
    expect(await settle(trial)).toBe('pending');
    document.getElementById('c2')?.focus();
    expect(typeof (await settle(trial))).toBe('number');
  });
});

describe('route_switch', () => {
  function build(selected: 'library' | 'settings') {
    document.body.innerHTML = `
      <button role="tab" id="r-trigger-library" aria-selected="${selected === 'library'}"></button>
      <button role="tab" id="r-trigger-settings" aria-selected="${selected === 'settings'}"></button>
      <div data-crosshook-page-scroll="true"><div id="page"></div></div>`;
  }

  it('focuses the non-current trigger and fires when the route content child is replaced', async () => {
    build('library');
    const focusInSeen: Event[] = [];
    const trial = (() => {
      document.addEventListener('focusin', (event) => focusInSeen.push(event));
      return arm('route_switch') as ScenarioTrial;
    })();
    expect(document.activeElement?.id).toBe('r-trigger-settings');
    // the activation-causing focusin is swallowed before it reaches the document
    expect(focusInSeen).toHaveLength(0);

    const root = document.querySelector('[data-crosshook-page-scroll="true"]') as HTMLElement;
    root.replaceChildren(document.createElement('div'));
    expect(typeof (await settle(trial))).toBe('number');
  });

  it('targets library when settings is the current route', () => {
    build('settings');
    arm('route_switch');
    expect(document.activeElement?.id).toBe('r-trigger-library');
  });
});

describe('health_table', () => {
  function content(): HTMLElement {
    return document.querySelector('[data-crosshook-page-scroll="true"]') as HTMLElement;
  }

  it('returns null without the health trigger or the content root', () => {
    document.body.innerHTML = '<div data-crosshook-page-scroll="true"></div>';
    expect(arm('health_table')).toBeNull();
    document.body.innerHTML = '<button role="tab" id="r-trigger-health"></button>';
    expect(arm('health_table')).toBeNull();
  });

  it('focuses the health trigger without activating it', () => {
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    const focusInSeen: Event[] = [];
    document.addEventListener('focusin', (event) => focusInSeen.push(event));
    arm('health_table');
    expect(document.activeElement?.id).toBe('r-trigger-health');
    expect(focusInSeen).toHaveLength(0);
  });

  it('fires when the live table reaches 300 rows', async () => {
    buildRoutes({ healthRows: 0, selected: 'library' });
    const trial = arm('health_table') as ScenarioTrial;
    content().innerHTML = '<div role="tabpanel"></div>';
    appendLiveTable(content(), HEALTH_MIN_ROWS);
    expect(typeof (await settle(trial))).toBe('number');
  });

  it('does not fire at 299 rows', async () => {
    buildRoutes({ healthRows: 0, selected: 'library' });
    const trial = arm('health_table') as ScenarioTrial;
    appendLiveTable(content(), HEALTH_MIN_ROWS - 1);
    expect(await settle(trial)).toBe('pending');
  });

  it('does not fire for a cached-only table', async () => {
    buildRoutes({ healthRows: 0, selected: 'library' });
    const trial = arm('health_table') as ScenarioTrial;
    content().insertAdjacentHTML('beforeend', healthTableHtml(CACHED_LABEL, 500));
    expect(await settle(trial)).toBe('pending');
  });

  it('reset reopens the library only when health is the selected route', async () => {
    const scenario = getScenario('health_table');
    buildRoutes({ healthRows: 1, selected: 'health' });
    await expect(scenario?.reset?.()).resolves.toBe(true);
    expect(document.getElementById('r-trigger-library')?.getAttribute('aria-selected')).toBe('true');
    expect(document.getElementById('r-content-library')).not.toBeNull();

    const root = content();
    const before = root.innerHTML;
    await expect(scenario?.reset?.()).resolves.toBe(true);
    expect(root.innerHTML).toBe(before); // already on the library: nothing activated
  });

  it('prepare reports the table, then returns to the library', async () => {
    buildRoutes({ healthRows: HEALTH_MIN_ROWS });
    await expect(getScenario('health_table')?.prepare?.()).resolves.toBe(true);
    expect(callCommandMock).toHaveBeenCalledWith(
      'bench_mark',
      expect.objectContaining({ scenario: 'health_table', phase: 'health_ready', seq: 0 })
    );
    expect(document.getElementById('r-trigger-library')?.getAttribute('aria-selected')).toBe('true');
  });

  it('prepare returns false at the timeout with too few rows', async () => {
    vi.useFakeTimers();
    try {
      buildRoutes({ healthRows: HEALTH_MIN_ROWS - 1 });
      const pending = getScenario('health_table')?.prepare?.() as Promise<boolean>;
      await vi.advanceTimersByTimeAsync(61_000);
      await expect(pending).resolves.toBe(false);
      expect(callCommandMock).toHaveBeenCalledWith(
        'bench_mark',
        expect.objectContaining({ scenario: 'health_table', phase: 'health_rows_too_few', seq: 0 })
      );
      expect(document.getElementById('r-trigger-health')?.getAttribute('aria-selected')).toBe('true');
    } finally {
      vi.useRealTimers();
    }
  });
});
