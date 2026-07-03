import { act, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, it } from 'vitest';
import { focusElement } from '@/hooks/gamepad-nav/dom';
import { useRovingTabindex } from '../useRovingTabindex';

interface HarnessProps {
  withDisabled?: boolean;
  mounted?: boolean;
}

function RovingItems({ withDisabled = false }: { withDisabled?: boolean }) {
  return (
    <>
      <button type="button" data-roving-item="">
        One
      </button>
      {withDisabled ? (
        <>
          <button type="button" data-roving-item="" disabled>
            Disabled
          </button>
          {/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: deliberate fixture — the hook must skip aria-hidden items and park them at tabIndex -1 */}
          <button type="button" data-roving-item="" aria-hidden="true">
            Hidden
          </button>
        </>
      ) : null}
      <button type="button" data-roving-item="">
        Two
      </button>
      <button type="button" data-roving-item="">
        Three
      </button>
    </>
  );
}

function RovingHarness({ withDisabled = false, mounted = true }: HarnessProps) {
  const rovingRef = useRovingTabindex({ itemSelector: '[data-roving-item]' });

  if (!mounted) {
    return null;
  }

  return (
    <div ref={rovingRef} data-testid="roving-container">
      <RovingItems withDisabled={withDisabled} />
    </div>
  );
}

function LateMountHarness() {
  const [mounted, setMounted] = useState(false);
  const rovingRef = useRovingTabindex({ itemSelector: '[data-roving-item]' });

  return (
    <>
      <button type="button" onClick={() => setMounted(true)}>
        Mount
      </button>
      {mounted ? (
        <div ref={rovingRef} data-testid="roving-container">
          <RovingItems />
        </div>
      ) : null}
    </>
  );
}

function rovingItems(): HTMLElement[] {
  return Array.from(screen.getByTestId('roving-container').querySelectorAll<HTMLElement>('[data-roving-item]'));
}

function itemsWithTabStop(): HTMLElement[] {
  return rovingItems().filter((item) => item.tabIndex === 0);
}

function pressKey(target: HTMLElement, key: string): void {
  act(() => {
    target.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true }));
  });
}

async function flushMutationObserver(): Promise<void> {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

describe('useRovingTabindex', () => {
  it('exactly one item has tabIndex 0 after mount', () => {
    render(<RovingHarness />);

    const [first, second, third] = rovingItems();
    expect(first.tabIndex).toBe(0);
    expect(second.tabIndex).toBe(-1);
    expect(third.tabIndex).toBe(-1);
    expect(itemsWithTabStop()).toHaveLength(1);
  });

  it('activates a container mounted after first render', () => {
    render(<LateMountHarness />);

    act(() => {
      screen.getByRole('button', { name: 'Mount' }).click();
    });

    const [first, second] = rovingItems();
    expect(itemsWithTabStop()).toHaveLength(1);
    expect(first.tabIndex).toBe(0);

    act(() => first.focus());
    pressKey(first, 'ArrowDown');
    expect(document.activeElement).toBe(second);
    expect(second.tabIndex).toBe(0);
  });

  it('ArrowDown and ArrowUp move focus and roving index', () => {
    render(<RovingHarness />);
    const [first, second] = rovingItems();
    act(() => first.focus());

    pressKey(first, 'ArrowDown');
    expect(document.activeElement).toBe(second);
    expect(second.tabIndex).toBe(0);
    expect(first.tabIndex).toBe(-1);

    pressKey(second, 'ArrowUp');
    expect(document.activeElement).toBe(first);
    expect(first.tabIndex).toBe(0);
    expect(second.tabIndex).toBe(-1);
  });

  it('Home and End jump to first and last', () => {
    render(<RovingHarness />);
    const [first, second, third] = rovingItems();
    act(() => second.focus());

    pressKey(second, 'End');
    expect(document.activeElement).toBe(third);
    expect(third.tabIndex).toBe(0);

    pressKey(third, 'Home');
    expect(document.activeElement).toBe(first);
    expect(first.tabIndex).toBe(0);
  });

  it('wraps from last to first and first to last', () => {
    render(<RovingHarness />);
    const [first, , third] = rovingItems();

    act(() => third.focus());
    pressKey(third, 'ArrowDown');
    expect(document.activeElement).toBe(first);

    pressKey(first, 'ArrowUp');
    expect(document.activeElement).toBe(third);
    expect(third.tabIndex).toBe(0);
  });

  it('dynamically added and removed items renormalize', async () => {
    render(<RovingHarness />);
    const container = screen.getByTestId('roving-container');
    const [first] = rovingItems();
    act(() => first.focus());

    const added = document.createElement('button');
    added.type = 'button';
    added.setAttribute('data-roving-item', '');
    added.textContent = 'Four';
    container.append(added);
    await flushMutationObserver();

    expect(itemsWithTabStop()).toHaveLength(1);
    expect(added.tabIndex).toBe(-1);

    // Removing the active item falls back to the first remaining item.
    first.remove();
    await flushMutationObserver();

    const remaining = rovingItems();
    expect(itemsWithTabStop()).toHaveLength(1);
    expect(remaining[0].tabIndex).toBe(0);
  });

  it('disabled and aria-hidden items are skipped', () => {
    render(<RovingHarness withDisabled />);
    const items = rovingItems();
    const first = items[0];
    const disabled = screen.getByRole('button', { name: 'Disabled', hidden: true });
    const hidden = items.find((item) => item.getAttribute('aria-hidden') === 'true');
    const second = screen.getByRole('button', { name: 'Two' });

    act(() => first.focus());
    pressKey(first, 'ArrowDown');

    expect(document.activeElement).toBe(second);
    expect(disabled.tabIndex).not.toBe(0);
    expect(hidden?.tabIndex).not.toBe(0);
  });

  it('gamepad focusElement focuses a tabIndex=-1 roving item', () => {
    render(<RovingHarness />);
    const [first, , third] = rovingItems();
    expect(first.tabIndex).toBe(0);
    expect(third.tabIndex).toBe(-1);

    act(() => {
      focusElement(third);
    });

    expect(document.activeElement).toBe(third);
    // focusin adoption makes it the roving active item.
    expect(third.tabIndex).toBe(0);
    expect(first.tabIndex).toBe(-1);
  });
});
