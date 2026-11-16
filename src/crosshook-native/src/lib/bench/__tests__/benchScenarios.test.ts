import { afterEach, describe, expect, it, vi } from 'vitest';
import { getScenario, type ScenarioTrial } from '../benchScenarios';

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
  it.each(['search', 'toggle_favorite', 'toggle_optimization', 'dpad', 'route_switch'])('%s returns null', (name) => {
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
