import { useEffect } from 'react';

const WHEEL_MULTIPLIER = 10;
const SMOOTH_FACTOR = 0.18;
const ARROW_SCROLL_PX = 80;
const SCROLLABLE = '.crosshook-content-area, .crosshook-console-drawer__body';

const INTERACTIVE_ROLES = new Set([
  'tablist', 'listbox', 'menu', 'menubar', 'radiogroup', 'slider',
  'combobox', 'select', 'spinbutton', 'tree', 'grid',
]);

function isInteractiveTarget(el: Element | null): boolean {
  if (!el) return false;
  const tag = el.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  if ((el as HTMLElement).isContentEditable) return true;
  const role = el.getAttribute('role');
  if (role && INTERACTIVE_ROLES.has(role)) return true;
  if (el.closest('[role="tablist"], [role="listbox"], [role="menu"], [role="combobox"]')) return true;
  // Radix Select triggers use data-state and aria-expanded
  if (el.hasAttribute('aria-expanded')) return true;
  return false;
}

export function useScrollEnhance(): void {
  useEffect(() => {
    // Accumulated velocity that gets smoothly drained each frame.
    let velocityX = 0;
    let velocityY = 0;
    let activeContainer: HTMLElement | null = null;
    let rafId = 0;

    function tick() {
      if (!activeContainer) return;

      activeContainer.scrollTop += velocityY * SMOOTH_FACTOR;
      activeContainer.scrollLeft += velocityX * SMOOTH_FACTOR;

      velocityY *= 1 - SMOOTH_FACTOR;
      velocityX *= 1 - SMOOTH_FACTOR;

      if (Math.abs(velocityY) > 0.5 || Math.abs(velocityX) > 0.5) {
        rafId = requestAnimationFrame(tick);
      } else {
        velocityX = 0;
        velocityY = 0;
        rafId = 0;
      }
    }

    function onWheel(e: WheelEvent) {
      if (!(e.target instanceof Element)) return;
      const container = e.target.closest(SCROLLABLE) as HTMLElement | null;
      if (!container) return;

      activeContainer = container;
      velocityY += e.deltaY * WHEEL_MULTIPLIER;
      velocityX += e.deltaX * WHEEL_MULTIPLIER;

      if (!rafId) {
        rafId = requestAnimationFrame(tick);
      }
    }

    function onKeyDown(e: KeyboardEvent) {
      if (e.defaultPrevented) return;
      if (isInteractiveTarget(document.activeElement)) return;

      let dx = 0;
      let dy = 0;

      switch (e.key) {
        case 'ArrowUp':    dy = -ARROW_SCROLL_PX; break;
        case 'ArrowDown':  dy = ARROW_SCROLL_PX;  break;
        case 'ArrowLeft':  dx = -ARROW_SCROLL_PX; break;
        case 'ArrowRight': dx = ARROW_SCROLL_PX;  break;
        default: return;
      }

      const container =
        (document.activeElement as Element | null)?.closest(SCROLLABLE) as HTMLElement | null
        ?? document.querySelector('.crosshook-content-area') as HTMLElement | null;
      if (!container) return;

      e.preventDefault();
      container.scrollBy({ top: dy, left: dx, behavior: 'smooth' });
    }

    document.addEventListener('wheel', onWheel, { passive: true });
    document.addEventListener('keydown', onKeyDown);

    return () => {
      document.removeEventListener('wheel', onWheel);
      document.removeEventListener('keydown', onKeyDown);
      if (rafId) cancelAnimationFrame(rafId);
    };
  }, []);
}
