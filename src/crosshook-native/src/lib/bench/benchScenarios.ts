/**
 * Bench-only G5 scenarios. Each scenario focuses its target ("arm"), then
 * reports the first matching DOM change with a `performance.now()` read inside
 * the observer/listener callback. Scenarios never inject input, never call
 * `preventDefault`, and never add DOM nodes or styles; every observer and
 * listener is removed by `ScenarioTrial.dispose`.
 */

import { activate, focusTarget, focusWithoutActivation, tabTrigger, waitForElement } from './benchDom';
import {
  CONTENT_ROOT,
  HEALTH_MIN_ROWS,
  HEALTH_TABLE_SCENARIO,
  LIBRARY_CONTENT,
  liveHealthRowCount,
  openRoute,
  reportHealthTable,
} from './healthTable';

export interface ScenarioTrial {
  /** Resolves with `performance.now()` read inside the callback that saw the change. */
  changed: Promise<number>;
  /** Removes every observer/listener this trial registered. Idempotent. */
  dispose: () => void;
}

export interface BenchScenario {
  /** One-time setup (reach the screen that holds the target). Resolves false when it cannot. */
  prepare?: (signal?: AbortSignal) => Promise<boolean>;
  /** Untimed setup before every `arm` (return to the starting screen). Resolves false when it cannot. */
  reset?: (signal?: AbortSignal) => Promise<boolean>;
  /** Focuses the target and starts watching for the change; null when the target is missing. */
  arm: () => ScenarioTrial | null;
}

const PREPARE_STEP_TIMEOUT_MS = 10_000;

function createTrial(attach: (fire: () => void) => () => void): ScenarioTrial {
  let cleanup: () => void = () => {};
  const changed = new Promise<number>((resolve) => {
    let fired = false;
    cleanup = attach(() => {
      if (fired) {
        return;
      }
      fired = true;
      resolve(performance.now());
    });
  });
  return { changed, dispose: () => cleanup() };
}

function mutationTrial(
  target: Node,
  init: MutationObserverInit,
  accept: (records: MutationRecord[]) => boolean = () => true
): ScenarioTrial {
  return createTrial((fire) => {
    const observer = new MutationObserver((records) => {
      if (accept(records)) {
        fire();
      }
    });
    observer.observe(target, init);
    return () => observer.disconnect();
  });
}

function eventTrial(target: EventTarget, type: string, accept: (event: Event) => boolean = () => true): ScenarioTrial {
  return createTrial((fire) => {
    const listener = (event: Event) => {
      if (accept(event)) {
        fire();
      }
    };
    target.addEventListener(type, listener);
    return () => target.removeEventListener(type, listener);
  });
}

const SEARCH_INPUT = 'input[aria-label="Search games"]';
const LIBRARY_TOOLBAR = '.crosshook-library-toolbar';
const FAVORITE_BUTTON = 'button[aria-pressed][aria-label^="Toggle favorite"]';
const FIRST_CARD_FOCUSABLE = '.crosshook-library-grid .crosshook-library-card__details-hitbox';
const GAMEPAD_ROOT = 'main.crosshook-focus-scope';
const OPTIMIZATION_CHECKBOX =
  '[role="tabpanel"][id$="-content-optimizations"] input.crosshook-launch-optimizations__checkbox:not(:disabled)';

// Setup path to the launch-optimizations checkboxes, as DOM activations:
// library card "View details" -> hero "Launch options" tab -> "Optimizations" sub-tab.
const OPTIMIZATION_SETUP_STEPS: readonly { selector: string; reached: string }[] = [
  { selector: 'button.crosshook-library-card__btn--details', reached: '[role="tab"][id$="-trigger-launch-options"]' },
  { selector: '[role="tab"][id$="-trigger-launch-options"]', reached: '[role="tab"][id$="-trigger-optimizations"]' },
  { selector: '[role="tab"][id$="-trigger-optimizations"]', reached: OPTIMIZATION_CHECKBOX },
];

const search: BenchScenario = {
  arm: () => {
    const input = document.querySelector<HTMLInputElement>(SEARCH_INPUT);
    const content = input?.closest(LIBRARY_CONTENT);
    if (!input || !content) {
      return null;
    }
    focusTarget(input);
    const valueAtArm = input.value;
    return mutationTrial(
      content,
      { childList: true, subtree: true },
      (records) =>
        input.value !== valueAtArm &&
        records.some((record) => !(record.target instanceof Element && record.target.closest(LIBRARY_TOOLBAR)))
    );
  },
};

const toggleFavorite: BenchScenario = {
  arm: () => {
    const button = document.querySelector<HTMLElement>(FAVORITE_BUTTON);
    if (!button) {
      return null;
    }
    focusTarget(button);
    return mutationTrial(button, { attributes: true, attributeFilter: ['aria-pressed'] });
  },
};

const toggleOptimization: BenchScenario = {
  prepare: async () => {
    for (const step of OPTIMIZATION_SETUP_STEPS) {
      if (document.querySelector(step.reached)) {
        continue;
      }
      const element = await waitForElement(step.selector, PREPARE_STEP_TIMEOUT_MS);
      if (!element) {
        return false;
      }
      activate(element);
    }
    return (await waitForElement(OPTIMIZATION_CHECKBOX, PREPARE_STEP_TIMEOUT_MS)) !== null;
  },
  arm: () => {
    const checkbox = document.querySelector<HTMLInputElement>(OPTIMIZATION_CHECKBOX);
    if (!checkbox) {
      return null;
    }
    focusTarget(checkbox);
    return eventTrial(checkbox, 'change');
  },
};

const dpad: BenchScenario = {
  arm: () => {
    const card = document.querySelector<HTMLElement>(FIRST_CARD_FOCUSABLE);
    if (!card) {
      return null;
    }
    const root = card.closest(GAMEPAD_ROOT) ?? document.body;
    focusTarget(card);
    return eventTrial(
      document,
      'focusin',
      (event) => event.target instanceof Node && event.target !== card && root.contains(event.target)
    );
  },
};

const routeSwitch: BenchScenario = {
  arm: () => {
    const libraryActive = tabTrigger('library')?.getAttribute('aria-selected') === 'true';
    const target = tabTrigger(libraryActive ? 'settings' : 'library');
    const contentRoot = document.querySelector(CONTENT_ROOT);
    if (!target || !contentRoot) {
      return null;
    }
    focusWithoutActivation(target);
    return mutationTrial(contentRoot, { childList: true });
  },
};

const healthTable: BenchScenario = {
  prepare: async (signal) =>
    (await reportHealthTable(HEALTH_TABLE_SCENARIO, signal)) && (await openRoute('library', signal)),
  // Back to the library before every `armed`, so each timed switch is library -> health.
  reset: async (signal) => {
    if (tabTrigger('health')?.getAttribute('aria-selected') !== 'true') {
      return true;
    }
    return openRoute('library', signal);
  },
  arm: () => {
    const trigger = tabTrigger('health');
    const contentRoot = document.querySelector(CONTENT_ROOT);
    if (!trigger || !contentRoot) {
      return null;
    }
    focusWithoutActivation(trigger);
    return mutationTrial(
      contentRoot,
      { childList: true, subtree: true },
      () => liveHealthRowCount(contentRoot) >= HEALTH_MIN_ROWS
    );
  },
};

const SCENARIOS: ReadonlyMap<string, BenchScenario> = new Map([
  ['search', search],
  ['toggle_favorite', toggleFavorite],
  ['toggle_optimization', toggleOptimization],
  ['dpad', dpad],
  ['route_switch', routeSwitch],
  [HEALTH_TABLE_SCENARIO, healthTable],
]);

/** Returns the scenario for an allowlisted name, or null for anything else. */
export function getScenario(name: string): BenchScenario | null {
  return SCENARIOS.get(name) ?? null;
}
