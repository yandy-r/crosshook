import { renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { focusElement } from '@/hooks/gamepad-nav/dom';
import { MOTION_ATTRIBUTE } from '@/lib/motion';
import { findEnhancedScrollContainer, SCROLL_ENHANCE_SELECTORS, useScrollEnhance } from '../useScrollEnhance';

function setScrollMetrics(
  el: HTMLElement,
  {
    clientHeight,
    scrollHeight,
    scrollTop = 0,
    clientWidth = 100,
    scrollWidth = 100,
    scrollLeft = 0,
  }: {
    clientHeight: number;
    scrollHeight: number;
    scrollTop?: number;
    clientWidth?: number;
    scrollWidth?: number;
    scrollLeft?: number;
  }
) {
  Object.defineProperties(el, {
    clientHeight: { configurable: true, value: clientHeight },
    scrollHeight: { configurable: true, value: scrollHeight },
    clientWidth: { configurable: true, value: clientWidth },
    scrollWidth: { configurable: true, value: scrollWidth },
  });
  el.scrollTop = scrollTop;
  el.scrollLeft = scrollLeft;
}

afterEach(() => {
  document.body.replaceChildren();
  document.documentElement.removeAttribute(MOTION_ATTRIBUTE);
});

function mountScrollFixture(): { container: HTMLElement; target: HTMLElement } {
  const container = document.createElement('div');
  container.className = 'crosshook-route-card-scroll';
  setScrollMetrics(container, { clientHeight: 100, scrollHeight: 400 });

  const target = document.createElement('button');
  container.append(target);
  document.body.append(container);
  return { container, target };
}

describe('useScrollEnhance selectors', () => {
  it('registers fill-mode subtab panel bodies for enhanced wheel scrolling', () => {
    const matches = SCROLL_ENHANCE_SELECTORS.match(/\.crosshook-subtab-content__inner--scroll\b/g);
    expect(matches?.length).toBe(1);
  });

  it('registers the context rail body scroll target exactly once', () => {
    const matches = SCROLL_ENHANCE_SELECTORS.match(/\.crosshook-context-rail__body\b/g);
    expect(matches?.length).toBe(1);
  });

  it('falls back to the nearest scrollable ancestor when a registered child cannot scroll', () => {
    const outer = document.createElement('div');
    outer.className = 'crosshook-route-card-scroll';
    setScrollMetrics(outer, { clientHeight: 100, scrollHeight: 400 });

    const child = document.createElement('section');
    child.className = 'crosshook-hero-detail__profiles-editor';
    setScrollMetrics(child, { clientHeight: 100, scrollHeight: 100 });

    const target = document.createElement('button');
    child.append(target);
    outer.append(child);
    document.body.append(outer);

    expect(findEnhancedScrollContainer(target, 0, 120)).toBe(outer);
  });

  it('keeps wheel input on the registered child when that child can scroll', () => {
    const outer = document.createElement('div');
    outer.className = 'crosshook-route-card-scroll';
    setScrollMetrics(outer, { clientHeight: 100, scrollHeight: 400 });

    const child = document.createElement('section');
    child.className = 'crosshook-hero-detail__profiles-editor';
    setScrollMetrics(child, { clientHeight: 100, scrollHeight: 240 });

    const target = document.createElement('button');
    child.append(target);
    outer.append(child);
    document.body.append(outer);

    expect(findEnhancedScrollContainer(target, 0, 120)).toBe(child);
  });
});

describe('useScrollEnhance reduced motion', () => {
  it('wheel under reduced motion applies delta synchronously without rAF', () => {
    document.documentElement.setAttribute(MOTION_ATTRIBUTE, 'reduced');
    const rafSpy = vi.spyOn(window, 'requestAnimationFrame');
    const { container, target } = mountScrollFixture();
    const { unmount } = renderHook(() => useScrollEnhance());

    target.dispatchEvent(new WheelEvent('wheel', { deltaY: 120, bubbles: true }));

    // WHEEL_MULTIPLIER is 2: the full delta lands in one synchronous step.
    expect(container.scrollTop).toBe(240);
    expect(rafSpy).not.toHaveBeenCalled();

    unmount();
    rafSpy.mockRestore();
  });

  it('arrow key scroll uses behavior auto under reduced motion', () => {
    const { container, target } = mountScrollFixture();
    const scrollBySpy = vi.fn();
    Object.defineProperty(container, 'scrollBy', { configurable: true, value: scrollBySpy });
    const { unmount } = renderHook(() => useScrollEnhance());
    target.focus();

    document.documentElement.setAttribute(MOTION_ATTRIBUTE, 'reduced');
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
    expect(scrollBySpy).toHaveBeenLastCalledWith({ top: 80, left: 0, behavior: 'auto' });

    // Global setup matchMedia mock reports no reduced-motion preference.
    document.documentElement.removeAttribute(MOTION_ATTRIBUTE);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true, cancelable: true }));
    expect(scrollBySpy).toHaveBeenLastCalledWith({ top: 80, left: 0, behavior: 'smooth' });

    unmount();
  });

  it('focusElement passes auto behavior under reduced motion', () => {
    const button = document.createElement('button');
    document.body.append(button);
    const scrollIntoView = vi.mocked(HTMLElement.prototype.scrollIntoView);
    scrollIntoView.mockClear();

    document.documentElement.setAttribute(MOTION_ATTRIBUTE, 'reduced');
    focusElement(button);
    expect(scrollIntoView).toHaveBeenLastCalledWith({ block: 'nearest', inline: 'nearest', behavior: 'auto' });

    document.documentElement.removeAttribute(MOTION_ATTRIBUTE);
    focusElement(button);
    expect(scrollIntoView).toHaveBeenLastCalledWith({
      block: 'nearest',
      inline: 'nearest',
      behavior: undefined,
    });
  });
});
