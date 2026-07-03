import { afterEach, describe, expect, it } from 'vitest';
import { isMotionReduced, MOTION_ATTRIBUTE, REDUCED_MOTION_QUERY, scrollBehavior } from '../motion';

type Restore = () => void;

function stubMatchMedia(matches: boolean): Restore {
  const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    writable: true,
    value: (query: string) => ({
      matches: query === REDUCED_MOTION_QUERY ? matches : false,
      media: query,
      onchange: null,
      addListener: () => {},
      removeListener: () => {},
      addEventListener: () => {},
      removeEventListener: () => {},
      dispatchEvent: () => true,
    }),
  });
  return () => {
    if (original) {
      Object.defineProperty(window, 'matchMedia', original);
    }
  };
}

function undefineMatchMedia(): Restore {
  const original = Object.getOwnPropertyDescriptor(window, 'matchMedia');
  Object.defineProperty(window, 'matchMedia', {
    configurable: true,
    writable: true,
    value: undefined,
  });
  return () => {
    if (original) {
      Object.defineProperty(window, 'matchMedia', original);
    }
  };
}

afterEach(() => {
  document.documentElement.removeAttribute(MOTION_ATTRIBUTE);
});

describe('isMotionReduced', () => {
  it('isMotionReduced returns true for attribute reduced', () => {
    document.documentElement.setAttribute(MOTION_ATTRIBUTE, 'reduced');
    expect(isMotionReduced()).toBe(true);
  });

  it('isMotionReduced returns false for attribute full even when media query matches', () => {
    const restore = stubMatchMedia(true);
    try {
      document.documentElement.setAttribute(MOTION_ATTRIBUTE, 'full');
      expect(isMotionReduced()).toBe(false);
    } finally {
      restore();
    }
  });

  it('isMotionReduced falls back to matchMedia when attribute absent', () => {
    const restore = stubMatchMedia(true);
    try {
      expect(isMotionReduced()).toBe(true);
    } finally {
      restore();
    }
  });

  it('isMotionReduced returns false without attribute and without matchMedia', () => {
    const restore = undefineMatchMedia();
    try {
      expect(isMotionReduced()).toBe(false);
    } finally {
      restore();
    }
  });
});

describe('scrollBehavior', () => {
  it('scrollBehavior maps reduced to auto and full to smooth', () => {
    document.documentElement.setAttribute(MOTION_ATTRIBUTE, 'reduced');
    expect(scrollBehavior()).toBe('auto');

    document.documentElement.setAttribute(MOTION_ATTRIBUTE, 'full');
    expect(scrollBehavior()).toBe('smooth');
  });
});
