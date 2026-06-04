import { afterEach, describe, expect, it } from 'vitest';
import { findEnhancedScrollContainer, SCROLL_ENHANCE_SELECTORS } from '../useScrollEnhance';

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
});

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
