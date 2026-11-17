import { afterEach, describe, expect, it, vi } from 'vitest';
import { activate, focusTarget, focusWithoutActivation, tabTrigger, waitForElement } from '../benchDom';

afterEach(() => {
  document.body.innerHTML = '';
  vi.useRealTimers();
});

describe('tabTrigger', () => {
  it('finds a Radix trigger by its value suffix', () => {
    document.body.innerHTML =
      '<button role="tab" id="r1-trigger-health"></button><button role="tab" id="r1-trigger-library"></button>';
    expect(tabTrigger('health')?.id).toBe('r1-trigger-health');
    expect(tabTrigger('missing')).toBeNull();
  });
});

describe('focus helpers', () => {
  it('focusTarget focuses without scrolling', () => {
    document.body.innerHTML = '<button id="b"></button>';
    const button = document.getElementById('b') as HTMLElement;
    const focus = vi.spyOn(button, 'focus');
    focusTarget(button);
    expect(focus).toHaveBeenCalledWith({ preventScroll: true });
    expect(document.activeElement).toBe(button);
  });

  it('focusWithoutActivation swallows the focusin and removes its listener', () => {
    document.body.innerHTML = '<button id="b"></button>';
    const button = document.getElementById('b') as HTMLElement;
    const seen = vi.fn();
    document.addEventListener('focusin', seen);
    focusWithoutActivation(button);
    expect(document.activeElement).toBe(button);
    expect(seen).not.toHaveBeenCalled();
    button.blur();
    focusTarget(button);
    expect(seen).toHaveBeenCalledTimes(1);
    document.removeEventListener('focusin', seen);
  });
});

describe('activate', () => {
  it('dispatches mousedown then click', () => {
    document.body.innerHTML = '<button id="b"></button>';
    const button = document.getElementById('b') as HTMLElement;
    const order: string[] = [];
    button.addEventListener('mousedown', () => order.push('mousedown'));
    button.addEventListener('click', () => order.push('click'));
    activate(button);
    expect(order).toEqual(['mousedown', 'click']);
  });
});

describe('waitForElement', () => {
  it('resolves an existing element at once', async () => {
    document.body.innerHTML = '<i class="x"></i>';
    await expect(waitForElement('.x', 1000)).resolves.toBe(document.querySelector('.x'));
  });

  it('resolves an element added later', async () => {
    const pending = waitForElement('.late', 2000);
    const el = document.createElement('div');
    el.className = 'late';
    document.body.append(el);
    await expect(pending).resolves.toBe(el);
  });

  it('resolves null on timeout', async () => {
    vi.useFakeTimers();
    const pending = waitForElement('.never', 500);
    await vi.advanceTimersByTimeAsync(500);
    await expect(pending).resolves.toBeNull();
  });

  it('resolves null on abort, and at once for an already-aborted signal', async () => {
    const controller = new AbortController();
    const pending = waitForElement('.never', 60_000, controller.signal);
    controller.abort();
    await expect(pending).resolves.toBeNull();

    document.body.innerHTML = '<i class="x"></i>';
    await expect(waitForElement('.x', 1000, controller.signal)).resolves.toBeNull();
  });
});
