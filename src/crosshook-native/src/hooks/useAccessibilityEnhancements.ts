import { useEffect } from 'react';

function applyAriaLabel(button: HTMLElement): void {
  if (button.getAttribute('aria-label') || button.getAttribute('aria-labelledby')) {
    return;
  }
  const title = button.getAttribute('title');
  const text = button.textContent?.trim();
  const derived = title?.trim() || text;
  if (derived) {
    button.setAttribute('aria-label', derived);
  }
}

function labelInteractiveElements(root: ParentNode): void {
  for (const button of root.querySelectorAll<HTMLElement>('button, [role="button"]')) {
    applyAriaLabel(button);
  }
}

export function useAriaLabelHydration(): void {
  useEffect(() => {
    if (typeof document === 'undefined') return;
    labelInteractiveElements(document);

    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        for (const node of mutation.addedNodes) {
          if (node instanceof HTMLElement) {
            labelInteractiveElements(node);
          }
        }
      }
    });

    observer.observe(document.body, { childList: true, subtree: true });
    return () => observer.disconnect();
  }, []);
}

export function useHighContrastTheme(enabled: boolean): void {
  useEffect(() => {
    if (typeof document === 'undefined') return;
    const root = document.documentElement;
    if (enabled) {
      root.setAttribute('data-crosshook-theme', 'high-contrast');
    } else {
      root.removeAttribute('data-crosshook-theme');
    }

    return () => {
      root.removeAttribute('data-crosshook-theme');
    };
  }, [enabled]);
}
