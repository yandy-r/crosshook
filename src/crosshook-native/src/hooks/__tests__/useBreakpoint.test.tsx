import { act, render, renderHook, screen, waitFor } from '@testing-library/react';
import { useRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireMatchMediaChangeListeners, MockResizeObserver, triggerResize } from '@/test/setup';
import { sizeFromWidth, type UseBreakpointResult, useBreakpoint } from '../useBreakpoint';

function setInnerWidth(w: number): void {
  Object.defineProperty(window, 'innerWidth', { value: w, configurable: true, writable: true });
}

function setInnerHeight(h: number): void {
  Object.defineProperty(window, 'innerHeight', { value: h, configurable: true, writable: true });
}

describe('sizeFromWidth', () => {
  it('buckets by PRD thresholds', () => {
    expect(sizeFromWidth(800)).toBe('deck');
    expect(sizeFromWidth(1099)).toBe('deck');
    expect(sizeFromWidth(1100)).toBe('narrow');
    expect(sizeFromWidth(1439)).toBe('narrow');
    expect(sizeFromWidth(1440)).toBe('desk');
    expect(sizeFromWidth(2199)).toBe('desk');
    expect(sizeFromWidth(2200)).toBe('uw');
    expect(sizeFromWidth(3440)).toBe('uw');
  });
});

function ShellWithBp() {
  const ref = useRef<HTMLDivElement | null>(null);
  const bp = useBreakpoint(ref);
  return (
    <div>
      <div data-testid="shell" ref={ref} style={{ display: 'block' }} />
      <span data-testid="bp-size">{bp.size}</span>
      <span data-testid="bp-uw">{String(bp.isUw)}</span>
    </div>
  );
}

describe('useBreakpoint', () => {
  let prevWidth: number;
  let prevHeight: number;

  beforeEach(() => {
    prevWidth = window.innerWidth;
    prevHeight = window.innerHeight;
  });

  afterEach(() => {
    setInnerWidth(prevWidth);
    setInnerHeight(prevHeight);
  });

  it('uses window size when no shell ref (deck below narrow)', () => {
    setInnerWidth(800);
    setInnerHeight(600);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current.size).toBe('deck');
    expect(result.current.isDeck).toBe(true);
    expect(result.current.isNarrow).toBe(false);
    expect(result.current.isDesk).toBe(false);
    expect(result.current.isUw).toBe(false);
  });

  it('reports narrow for 1100–1439', () => {
    setInnerWidth(1200);
    setInnerHeight(800);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current.size).toBe('narrow');
    expect(result.current.isNarrow).toBe(true);
  });

  it('reports desk for 1440–2199', () => {
    setInnerWidth(1920);
    setInnerHeight(1080);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current.size).toBe('desk');
    expect(result.current.isDesk).toBe(true);
  });

  it('reports uw for >= 2200', () => {
    setInnerWidth(3440);
    setInnerHeight(1440);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current.size).toBe('uw');
    expect(result.current.isUw).toBe(true);
  });

  it('returns static desk when matchMedia is missing', () => {
    const realMm = window.matchMedia;
    try {
      Object.defineProperty(window, 'matchMedia', {
        value: undefined,
        configurable: true,
        writable: true,
      });
      const { result } = renderHook(() => useBreakpoint());
      const desk: UseBreakpointResult = {
        size: 'desk',
        width: 1440,
        height: 900,
        isDeck: false,
        isNarrow: false,
        isDesk: true,
        isUw: false,
      };
      expect(result.current).toEqual(desk);
    } finally {
      Object.defineProperty(window, 'matchMedia', { value: realMm, configurable: true, writable: true });
    }
  });

  it('removes matchMedia listeners on unmount', () => {
    const remove = vi.fn();
    const mql: MediaQueryList = {
      media: '',
      matches: false,
      onchange: null,
      addEventListener: vi.fn(),
      removeEventListener: remove,
      addListener: vi.fn(),
      removeListener: remove,
      dispatchEvent: () => true,
    } as unknown as MediaQueryList;
    const mm = vi.spyOn(window, 'matchMedia').mockImplementation(() => mql);
    const { unmount } = renderHook(() => useBreakpoint());
    unmount();
    expect(remove).toHaveBeenCalled();
    mm.mockRestore();
  });

  it('updates from deck to desk when innerWidth changes after a matchMedia change event', async () => {
    setInnerWidth(800);
    setInnerHeight(600);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current.size).toBe('deck');

    setInnerWidth(1920);
    setInnerHeight(1080);
    await act(async () => {
      fireMatchMediaChangeListeners('(min-width: 1100px)');
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 40));
    });
    expect(result.current.size).toBe('desk');
    expect(result.current.isDesk).toBe(true);
  });

  it('updates from deck to narrow when innerWidth crosses 1100 after a matchMedia change event', async () => {
    setInnerWidth(800);
    setInnerHeight(600);
    const { result } = renderHook(() => useBreakpoint());
    expect(result.current.size).toBe('deck');

    setInnerWidth(1200);
    setInnerHeight(800);
    await act(async () => {
      fireMatchMediaChangeListeners('(min-width: 1100px)');
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 40));
    });
    expect(result.current.size).toBe('narrow');
    expect(result.current.isNarrow).toBe(true);
  });

  it('calls ResizeObserver.disconnect on unmount when shell ref observes an element', () => {
    const disconnectSpy = vi.spyOn(MockResizeObserver.prototype, 'disconnect');
    const { unmount } = render(<ShellWithBp />);
    unmount();
    expect(disconnectSpy).toHaveBeenCalled();
    disconnectSpy.mockRestore();
  });

  it('updates to uw when shell ResizeObserver reports 3440px width', async () => {
    render(<ShellWithBp />);
    const shell = screen.getByTestId('shell');
    vi.spyOn(shell, 'getBoundingClientRect').mockReturnValue({
      width: 3440,
      height: 200,
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      right: 3440,
      bottom: 200,
      toJSON: () => ({}),
    } as DOMRect);

    await act(async () => {
      triggerResize(shell, 3440, 200);
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 40));
    });
    await waitFor(() => {
      expect(screen.getByTestId('bp-size').textContent).toBe('uw');
      expect(screen.getByTestId('bp-uw').textContent).toBe('true');
    });
  });
});
