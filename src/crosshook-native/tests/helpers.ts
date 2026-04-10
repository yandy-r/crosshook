import type { Page } from '@playwright/test';

export interface ConsoleCapture {
  errors: string[];
}

export function attachConsoleCapture(page: Page): ConsoleCapture {
  const capture: ConsoleCapture = { errors: [] };
  page.on('pageerror', (err) => {
    capture.errors.push(`pageerror: ${err.message}`);
  });
  page.on('console', (msg) => {
    if (msg.type() === 'error') {
      capture.errors.push(`console.error: ${msg.text()}`);
    }
  });
  return capture;
}
